use anyhow::{anyhow, Result};
use async_openai::{
    types::{ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, Role},
    Client,
};
use indoc::formatdoc;
use tracing::debug;
use url::Url;

use crate::Action;

/// A conversation with GPT-4.
#[derive(Debug)]
pub struct Conversation {
    /// The goal for the agent to achieve.
    goal: String,
    /// The client used to communicate with OpenAI.
    client: Client,
    /// The URL of the current page.
    url: Option<Url>,
    /// A collection of messages sent to GPT-4.
    messages: Vec<ChatCompletionRequestMessage>,
}

impl Default for Conversation {
    fn default() -> Self {
        Self {
            goal: String::from("Visit 10 webpages."),
            url: None,
            client: Client::new(),
            messages: vec![ChatCompletionRequestMessage {
                name: None,
                role: Role::System,
                content: formatdoc!("
                    You are an agent controlling a browser. You are given the URL of the current website, and a simplified markup description of the page contents, which looks like this:
                    <p id=0>text</p>
                    <link id=1 href=\"link url\">text</link>
                    <button id=2>text</button>
                    <input id=3>placeholder</input>
                    <img id=4 alt=\"image description\"/>

                    You are not given a goal but should create and alter a goal based on the previous actions you have taken. Your initial goal should be to visit at least 10 webpages and update your goal based on the content of those page.

                    You must respond with ONLY one of the following commands AND NOTHING ELSE:
                        - CLICK X - click on a given element. You can only click on links, buttons, and inputs!
                        - TYPE X \"TEXT\" - type the specified text into the input with id X and press ENTER
                        - GOAL \"TEXT\" - Outputs your updated goal.
                "),
        }]}
    }
}

impl Conversation {
    /// Create a new conversation with GPT-4.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Request and execute an action from GPT-4.
    #[tracing::instrument]
    pub async fn request_action(&mut self, url: &str, page_content: &str) -> Result<Action> {
        self.enforce_context_length(url)?;

        self.messages.push(ChatCompletionRequestMessage {
            name: None,
            role: Role::User,
            content: format!(
                "OBJECTIVE: {}\nCURRENT URL: {url}\nPAGE CONTENT: {page_content}",
                self.goal
            ),
        });

        let response = self
            .client
            .chat()
            .create(
                CreateChatCompletionRequestArgs::default()
                    .model("gpt-4")
                    .temperature(0.7f32)
                    .max_tokens(100u16)
                    .messages(self.messages.clone())
                    .build()?,
            )
            .await?;

        debug!(
            "Got a response, used {} tokens.",
            response
                .usage
                .expect("Usage should be present.")
                .total_tokens
        );

        let message = &response
            .choices
            .get(0)
            .ok_or_else(|| anyhow!("No choices returned from OpenAI.",))?
            .message;

        self.messages.push(ChatCompletionRequestMessage {
            name: None,
            role: message.role.clone(),
            content: message.content.clone(),
        });

        message.content.clone().try_into()
    }

    fn enforce_context_length(&mut self, url: &str) -> Result<()> {
        let new_url = Url::parse(url)?;

        if self.url.as_ref().map(Url::host) != Some(new_url.host()) {
            debug!("Host changed, clearing context.");
            self.messages = self.messages.drain(..1).collect();
        }

        self.url = Some(new_url);
        Ok(())
    }
}
