use dotenv;
use itertools::Itertools;
use std::{collections::HashMap, fs, thread, time};
use twitch_irc::{
  login::StaticLoginCredentials, message::ServerMessage, ClientConfig, SecureTCPTransport,
  TwitchIRCClient,
};

fn get_env_var(name: &str) -> String {
  return dotenv::var(name).expect(&format!("Couldn't find {} variable in the .env file", name));
}

fn word_can_be_formed(letters: &HashMap<u8, usize>, word: &HashMap<u8, usize>) -> bool {
  word
    .iter()
    .all(|e| letters.contains_key(e.0) && &letters[e.0] >= e.1)
}

fn get_possible_words(word: &str, word_list_choice: &str) -> Result<Vec<String>, std::io::Error> {
  let text = fs::read_to_string(format!("./word-lists/{}.txt", word_list_choice))?;

  let word_list = text
    .split('\n')
    .filter(|w| w.trim() != "")
    .map(|w| w.to_lowercase());

  let letters = word.to_lowercase().bytes().counts();

  let min_count = 4;
  let max_count = word.len();

  let selected = word_list
    .filter(|w| {
      (min_count..=max_count).contains(&w.len())
        && word_can_be_formed(&letters, &w.bytes().counts())
    })
    .sorted()
    .sorted_by(|a, b| Ord::cmp(&a.len(), &b.len()))
    .collect_vec();

  Ok(selected)
}

#[tokio::main]
pub async fn main() {
  let login_name = get_env_var("USERNAME");
  let oauth_token = get_env_var("OAUTH_TOKEN");
  let channels = get_env_var("CHANNELS")
    .split(',')
    .map(|channel| channel.to_string().to_lowercase())
    .collect_vec();
  let bot_prefix = get_env_var("BOT_PREFIX");

  let config = ClientConfig::new_simple(StaticLoginCredentials::new(
    login_name.to_owned(),
    Some(oauth_token),
  ));

  let (mut incoming_messages, client) =
    TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

  for channel in channels {
    client.join(channel)
  }

  let join_handle = tokio::spawn(async move {
    while let Some(message) = incoming_messages.recv().await {
      match message {
        ServerMessage::Privmsg(msg) => {
          println!(
            "#{} -> {}: {}",
            msg.channel_login, msg.sender.name, msg.message_text
          );

          let sender_is_mod = msg
            .badges
            .iter()
            .any(|badge| ["moderator", "broadcaster"].contains(&&badge.name[..]));

          let words = msg
            .message_text
            .split_whitespace()
            .map(str::to_string)
            .collect_vec();

          if !msg.message_text.starts_with(&bot_prefix) || !sender_is_mod {
            continue;
          }

          if let Some(command) = words.get(0) {
            match &command.to_lowercase()[1..] {
              "guess" => {
                if let Some(word) = words.get(1) {
                  if let Some(word_list_choice) = words.get(2) {
                    if let Ok(possible_words) = get_possible_words(word, word_list_choice) {
                      // for w in possible_words {
                      //   client.say(msg.channel_login.clone(), w).await.unwrap();
                      //   thread::sleep(time::Duration::from_millis(250));
                      // }

                      client
                        .say(msg.channel_login.clone(), possible_words.join(" "))
                        .await
                        .unwrap();
                    }
                  }
                }
              }
              _ => {}
            }
          }
        }
        _ => {}
      }
    }
  });

  join_handle.await.unwrap();
}
