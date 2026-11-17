//! BankerNPC's `BankerDialogueLabellerFile` and the companion's
//! `canAccessBank` it calls, written against the native DialogueLabeller.
//! [`talk_to`] is the "talk-to" listener body
//! (`DialogueLabeller.open(player, BankerDialogueLabellerFile(), npc)`).
use super::{
    ChatAnim, Dialogue, Host, NodeHandle, Result, Text,
    labeller::{self, Conversation, DialogueOption, Dsl},
    text, texts,
};
use std::sync::Arc;

/// `NPCs.SIRSAL_BANKER_4519`.
pub const SIRSAL_BANKER: i32 = 4519;
/// `NPCs.JADE_4296`.
pub const JADE: i32 = 4296;
/// `NPCs.BANK_TUTOR_4907`.
pub const BANK_TUTOR: i32 = 4907;
/// `NPCs.BANKER_6538`, the Bounty Hunter banker.
pub const BOUNTY_BANKER: i32 = 6538;
/// `NPCs.ODOVACAR_5383`.
pub const ODOVACAR: i32 = 5383;
/// `BANKERS_WITH_EXTRA_OPTION`.
pub const BANKERS_WITH_EXTRA_OPTION: [i32; 4] = [BANK_TUTOR, JADE, BOUNTY_BANKER, ODOVACAR];
/// `Items.COINS_995` and `ODOVACAR_FEE`.
pub const COINS: i32 = 995;
pub const ODOVACAR_FEE: i32 = 100;

/// `SecondaryBankAccountActivationResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    AlreadyActive,
    InternalFailure,
    NotEnoughMoney,
    Success,
}

/// The ContentAPI and configuration reads the conversation makes.
pub trait BankHost: Host {
    /// `hasSealOfPassage(player)`.
    fn has_seal_of_passage(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `hasIronmanRestriction(player, IronmanMode.ULTIMATE)`.
    fn has_ultimate_ironman_restriction(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `hasAwaitingGrandExchangeCollections(player)`.
    fn has_awaiting_grand_exchange_collections(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `openBankAccount(player)`.
    fn open_bank_account(&mut self, player: &NodeHandle) -> Result<()>;
    /// `activateSecondaryBankAccount(player)`.
    fn activate_secondary_bank_account(&mut self, player: &NodeHandle) -> Result<Activation>;
    /// `toggleBankAccount(player)`.
    fn toggle_bank_account(&mut self, player: &NodeHandle) -> Result<()>;
    /// `isUsingSecondaryBankAccount(player)`.
    fn is_using_secondary_bank_account(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `hasActivatedSecondaryBankAccount(player)`.
    fn has_activated_secondary_bank_account(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `openBankPinSettings(player)`.
    fn open_bank_pin_settings(&mut self, player: &NodeHandle) -> Result<()>;
    /// `openGrandExchangeCollectionBox(player)`.
    fn open_grand_exchange_collection_box(&mut self, player: &NodeHandle) -> Result<()>;
    /// `removeItem(player, item)` from the inventory.
    fn remove_item(&mut self, player: &NodeHandle, item: &Self::Item) -> Result<bool>;
    /// ContentAPI `sendMessage(player, message)`.
    fn api_message(&mut self, player: &NodeHandle, message: Text) -> Result<()>;
    /// `ServerConstants.SECOND_BANK`.
    fn second_bank(&mut self) -> Result<bool>;
}

/// `BankerNPC.canAccessBank(player, npc)`: Odovacar charges 100 coins.
pub fn can_access_bank<H: BankHost>(
    host: &mut H,
    player: &NodeHandle,
    npc: &H::Npc,
) -> Result<bool> {
    if host.npc_id(npc)? != ODOVACAR {
        return Ok(true);
    }
    let fee = host.new_item_amount(COINS, ODOVACAR_FEE)?;
    if host.remove_item(player, &fee)? {
        host.api_message(
            player,
            text("You pay 100 coins to use the travelling bank."),
        )?;
        return Ok(true);
    }
    Ok(false)
}

/// The "talk-to" listener: `DialogueLabeller.open(player,
/// BankerDialogueLabellerFile(), npc)`.
pub fn talk_to<H: BankHost>(host: &mut H, player: &NodeHandle, npc: &H::Npc) -> Result<bool> {
    let file = labeller::file(BankerConversation);
    labeller::open(host, player, &Dialogue::File(file), npc)?;
    Ok(true)
}

/// `BankerDialogueLabellerFile`.
pub struct BankerConversation;

fn id_is<H: BankHost>(
    id: i32,
) -> impl Fn(&mut H, &NodeHandle, &H::Npc) -> Result<bool> + Send + Sync + 'static {
    move |host, _, npc| Ok(host.npc_id(npc)? == id)
}

impl<H: BankHost> Conversation<H> for BankerConversation {
    fn class_name(&self) -> &str {
        "BankerDialogueLabellerFile"
    }
    fn add_conversation(&self, host: &mut H, c: &Dsl<'_, H>) -> Result<()> {
        let npc =
            |host: &mut H, anim: ChatAnim, m: &[&str]| c.npc_lines(host, anim, &texts(m), false);
        let player =
            |host: &mut H, anim: ChatAnim, m: &[&str]| c.player_lines(host, anim, &texts(m), false);
        let n = ChatAnim::Neutral;
        c.exec(host, |host, p, npc| {
            if host.npc_id(npc)? == SIRSAL_BANKER && !host.has_seal_of_passage(p)? {
                c.load_label(p, "no seal of passage");
            }
            if host.has_ultimate_ironman_restriction(p)? {
                if host.npc_id(npc)? == JADE {
                    c.load_label(p, "uim for jade");
                }
                c.load_label(p, "uim");
            }
            if host.npc_id(npc)? == JADE {
                c.load_label(p, "hello for jade");
            }
            c.load_label(p, "hello");
            Ok(())
        })?;
        c.label(&["no seal of passage"]);
        npc(
            host,
            ChatAnim::Annoyed,
            &["What are you doing here, Fremennik?!"],
        )?;
        player(host, ChatAnim::Worried, &["I have a Seal of Pass..."])?;
        npc(host, ChatAnim::Angry, &["No you don't! Begone!"])?;
        c.goto("nowhere");
        c.label(&["uim for jade"]);
        npc(
            host,
            ChatAnim::Annoyed,
            &[
                "Greetings, warrior. I wish I could help you, but our services are not available for Ultimate Iron@g[men,women].",
            ],
        )?;
        c.goto("nowhere");
        c.label(&["uim"]);
        npc(
            host,
            ChatAnim::Annoyed,
            &[
                "My apologies, dear @g[sir,madam], our services are not available for Ultimate Iron@g[men,women]",
            ],
        )?;
        c.goto("nowhere");
        let ge_or_main = |host: &mut H, p: &NodeHandle, _: &H::Npc| {
            let label = if host.has_awaiting_grand_exchange_collections(p)? {
                "ge collect"
            } else {
                "main options"
            };
            c.load_label(p, label);
            Ok(())
        };
        c.label(&["hello for jade"]);
        npc(host, n, &["Greetings warrior, how may I help you?"])?;
        c.exec(host, ge_or_main)?;
        c.label(&["hello"]);
        npc(
            host,
            n,
            &["Good day, would you like to access your bank account?"],
        )?;
        c.exec(host, ge_or_main)?;
        c.label(&["ge collect"]);
        npc(
            host,
            n,
            &[
                "Before we go any further, I should inform you that you have items ready for collection from the Grand Exchange.",
            ],
        )?;
        c.goto("main options");

        c.label(&["main options"]);
        let main = [
            DialogueOption::new("how to use", "How do I use the bank?")
                .only_if(id_is::<H>(BANK_TUTOR)),
            DialogueOption::new("who is the bounty hunter banker", "Who are you?")
                .only_if(id_is::<H>(BOUNTY_BANKER)),
            DialogueOption::new("who is odovacar", "Who are you?").only_if(id_is::<H>(ODOVACAR)),
            DialogueOption::new("access", "I'd like to access my bank account please.")
                .expression(ChatAnim::Asking),
            DialogueOption::new(
                "buy second bank",
                "I'd like to open a secondary bank account.",
            )
            .only_if(|host: &mut H, p, _| {
                Ok(host.second_bank()? && !host.has_activated_secondary_bank_account(p)?)
            }),
            DialogueOption::new(
                "switch second bank",
                "I'd like to switch to my primary bank account.",
            )
            .only_if(|host: &mut H, p, _| {
                Ok(host.has_activated_secondary_bank_account(p)?
                    && host.is_using_secondary_bank_account(p)?)
            }),
            DialogueOption::new(
                "switch second bank",
                "I'd like to switch to my secondary bank account.",
            )
            .only_if(|host: &mut H, p, _| {
                Ok(host.has_activated_secondary_bank_account(p)?
                    && !host.is_using_secondary_bank_account(p)?)
            }),
            DialogueOption::new("pin", "I'd like to check my PIN settings."),
            DialogueOption::new("collect", "I'd like to collect items."),
            DialogueOption::new("what is this place", "What is this place?").only_if(
                |host: &mut H, _, npc| Ok(!BANKERS_WITH_EXTRA_OPTION.contains(&host.npc_id(npc)?)),
            ),
            DialogueOption::new(
                "jade's employment history",
                "How long have you worked here?",
            )
            .only_if(id_is::<H>(JADE)),
            DialogueOption::new(
                "is the bounty hunter banker afraid",
                "Aren't you afraid of working in the Wilderness?",
            )
            .only_if(|host: &mut H, _, npc| {
                Ok(host.npc_id(npc)? == BOUNTY_BANKER && !host.second_bank()?)
            }),
        ];
        c.options(host, &main.map(Arc::new), "Select an Option", false)?;

        c.label(&["how to use"]);
        let how = [
            DialogueOption::new("using bank", "Using the bank itself.")
                .spoken("Using the bank itself. I'm not sure how....?"),
            DialogueOption::new("using deposit", "Using bank deposit boxes.")
                .spoken("Using Bank deposit boxes.... what are they?"),
            DialogueOption::new(
                "using PIN",
                "What's this PIN thing that people keep talking about?",
            )
            .spoken("What's this PIN thing that people keep talking about?"),
            DialogueOption::new("nowhere", "Goodbye."),
        ];
        c.options(host, &how.map(Arc::new), "Select an Option", false)?;

        c.label(&["using bank"]);
        npc(
            host,
            n,
            &[
                "Speak to any banker and ask to see your bank",
                "account. If you have set a PIN you will be asked for",
                "it, then all the belongings you have placed in the bank",
                "will appear in the window. To withdraw one item, left-",
            ],
        )?;
        npc(host, n, &["click on it once."])?;
        npc(
            host,
            n,
            &[
                "To withdraw many, right-click on the item and select",
                "from the menu. The same for depositing, left-click on",
                "the item in your inventory to deposit it in the bank.",
                "Right-click on it to deposit many of the same items.",
            ],
        )?;
        npc(
            host,
            n,
            &[
                "To move things around in your bank: firstly select",
                "Swap or Insert as your default moving mode, you can",
                "find these buttons on the bank window itself. Then click",
                "and drag an item to where you want it to appear.",
            ],
        )?;
        npc(
            host,
            n,
            &[
                "You may withdraw 'notes' or 'certificates' when the",
                "items you are trying to withdraw do not stack in your",
                "inventory. This will only work for items which are",
                "tradeable.",
            ],
        )?;
        npc(
            host,
            n,
            &[
                "For instance, if you wanted to sell 100 logs to another",
                "player, they would not fit in one inventory and you",
                "would need to do multiple trades. Instead, click the",
                "Note button to withdraw the logs as 'certs' or 'notes'",
            ],
        )?;
        npc(host, n, &["then withdraw the items you need."])?;
        c.goto("how to use");

        c.label(&["using deposit"]);
        npc(
            host,
            n,
            &[
                "They look like grey pillars, there's one just over there,",
                "near the desk. Bank deposit boxes save so much time.",
                "If you're simply wanting to deposit a single item, 'Use'",
                "it on the deposit box.",
            ],
        )?;
        npc(
            host,
            n,
            &[
                "Otherwise, simply click once on the box and it will give",
                "you a choice of what to deposit in an interface very",
                "similar to the bank itself. Very quick for when you're",
                "simply fishing or mining etc.",
            ],
        )?;
        c.goto("how to use");

        c.label(&["using PIN"]);
        npc(
            host,
            n,
            &[
                "The PIN - Personal Identification Number - can be",
                "set on your bank account to protect the items there in",
                "case someone finds out your account password. It",
                "consists of four numbers that you remember and tell",
            ],
        )?;
        npc(host, n, &["no one."])?;
        npc(
            host,
            n,
            &[
                "So if someone did manage to get your password they",
                "couldn't steal your items if they were in the bank.",
            ],
        )?;
        c.goto("how to use");

        c.label(&["who is the bounty hunter banker"]);
        npc(
            host,
            n,
            &[
                "How inconsiderate of me, dear @g[sir,madam]. My name is Maximillian Sackville and I conduct operations here on behalf of The Bank of Gielinor.",
            ],
        )?;
        c.goto("main options");

        c.label(&["who is odovacar"]);
        npc(
            host,
            n,
            &[
                "How frightfully rude of me, my dear @g[chap,lady]. My name is Odovacar and I work for that excellent enterprise, the Bank of Gielinor.",
            ],
        )?;
        player(
            host,
            n,
            &["If you work for the bank, what are you doing here?"],
        )?;
        npc(
            host,
            n,
            &[
                "As part of our ongoing service to provide you, the customer, with an absolutely world-class banking experience, we are investigating the option of opening a chain of travelling banks.",
            ],
        )?;
        npc(
            host,
            n,
            &[
                "These will provide you, our esteemed customer, with the convenience of having banking facilities where they will be of optimum use to you. Such as here!",
            ],
        )?;
        player(host, n, &["Huh?"])?;
        npc(
            host,
            n,
            &[
                "I am the first of a new generation of travelling bankers that will wander the perilous areas of the world to provide you, the valued customer, with a bank when you need it most!",
            ],
        )?;
        player(
            host,
            n,
            &["So I can access my bank account simply by talking to you?"],
        )?;
        npc(host, n, &["Absolutely correct, dear @g[sir,lady]."])?;
        npc(
            host,
            n,
            &[
                "I must warn you, however, that due to the significantly increased overheads of an enterprise such as this, there is a small bank charge of 100gp every time you want to access your account.",
            ],
        )?;
        let fee = [
            DialogueOption::new(
                "access",
                "Sounds fair; here's the money. Can I access my account now?",
            ),
            DialogueOption::new(
                "payafterbank",
                "Let me open my account and then I'll pay you.",
            ),
            DialogueOption::new(
                "nopay2bank",
                "That's preposterous! I'm not paying to withdraw my own money!",
            ),
        ];
        c.options(host, &fee.map(Arc::new), "Select an Option", false)?;

        c.label(&["payafterbank"]);
        npc(
            host,
            n,
            &[
                "It's not that I don't trust you, old @g[chap,girl], but as the old adage goes, 'money comes before friends'.",
            ],
        )?;
        c.goto("nowhere");

        c.label(&["nopay2bank"]);
        npc(
            host,
            n,
            &[
                "I'm sorry to hear that, @g[sir,madam]. If you should reconsider, because I believe this service offers excellent value for money, do not hesitate to contact me.",
            ],
        )?;
        c.goto("nowhere");

        c.label(&["nomoney"]);
        npc(
            host,
            n,
            &[
                "I'm afraid you don't have the necessary funds with you at this time so I can't allow you to access your account. Please come again, when you have 100 gold to cover the fee.",
            ],
        )?;
        c.goto("nowhere");

        c.label(&["access"]);
        c.exec(host, |host, p, npc| {
            if !can_access_bank(host, p, npc)? {
                c.goto("nomoney");
            } else {
                host.open_bank_account(p)?;
                c.goto("nowhere");
            }
            Ok(())
        })?;

        c.label(&["buy second bank"]);
        npc(
            host,
            n,
            &["Certainly. We offer secondary accounts to all our customers."],
        )?;
        npc(
            host,
            n,
            &[
                "The secondary account comes with a standard fee of 5,000,000 coins. The fee is non-refundable and account activation is permanent.",
            ],
        )?;
        npc(
            host,
            n,
            &[
                "If your inventory does not contain enough money to cover the costs, we will complement the amount with the money inside your primary bank account.",
            ],
        )?;
        npc(
            host,
            n,
            &[
                "Knowing all this, would you like to proceed with opening your secondary bank account?",
            ],
        )?;
        let buy = [
            DialogueOption::new("buy second bank yes", "Yes, I am still interested.")
                .expression(ChatAnim::Happy),
            DialogueOption::new("buy second bank no", "Actually, I've changed my mind.")
                .expression(ChatAnim::Annoyed),
        ];
        c.options(host, &buy.map(Arc::new), "Select an Option", false)?;

        c.label(&["buy second bank yes"]);
        c.exec(host, |host, p, _| {
            let label = match host.activate_secondary_bank_account(p)? {
                Activation::AlreadyActive => "already",
                Activation::InternalFailure => "failure",
                Activation::NotEnoughMoney => "not enough cash",
                Activation::Success => "success",
            };
            c.load_label(p, label);
            Ok(())
        })?;
        c.label(&["already"]);
        npc(
            host,
            ChatAnim::Friendly,
            &["Your bank account was already activated, there is no need to pay twice."],
        )?;
        c.goto("main options");
        c.label(&["failure"]);
        npc(
            host,
            ChatAnim::Annoyed,
            &[
                "I must apologize, the transaction was not successful. Please check your primary bank account and your inventory - if there's money missing, please screenshot your chatbox and contact the game developers.",
            ],
        )?;
        c.goto("main options");
        c.label(&["not enough cash"]);
        npc(
            host,
            ChatAnim::Annoyed,
            &[
                "It appears that you do not have the money necessary to cover the costs associated with opening a secondary bank account. I will be waiting here until you do.",
            ],
        )?;
        c.goto("main options");
        c.label(&["success"]);
        npc(
            host,
            ChatAnim::Friendly,
            &[
                "Your secondary bank account has been opened and can be accessed through any of the Bank of Gielinor's employees. Thank you for choosing our services.",
            ],
        )?;
        c.goto("main options");

        c.label(&["buy second bank no"]);
        npc(
            host,
            n,
            &[
                "Very well. Should you decide a secondary bank account is needed, do not hesitate to contact any of the Bank of Gielinor's stationary employees. We will be happy to help.",
            ],
        )?;
        c.goto("main options");

        c.label(&["switch second bank"]);
        c.exec(host, |host, p, _| {
            host.toggle_bank_account(p)?;
            let label = if host.is_using_secondary_bank_account(p)? {
                "now secondary"
            } else {
                "now primary"
            };
            c.load_label(p, label);
            Ok(())
        })?;
        c.label(&["now primary"]);
        npc(
            host,
            n,
            &[
                "Your active bank account has been switched. You can now access your primary account.",
            ],
        )?;
        c.goto("main options");
        c.label(&["now secondary"]);
        npc(
            host,
            n,
            &[
                "Your active bank account has been switched. You can now access your secondary account.",
            ],
        )?;
        c.goto("main options");

        c.label(&["pin"]);
        c.exec(host, |host, p, _| host.open_bank_pin_settings(p))?;
        c.goto("nowhere");

        c.label(&["collect"]);
        c.exec(host, |host, p, _| {
            host.open_grand_exchange_collection_box(p)
        })?;
        c.goto("nowhere");

        c.label(&["what is this place"]);
        npc(
            host,
            n,
            &["This is a branch of the Bank of Gielinor. We have branches in many towns."],
        )?;
        player(host, n, &["And what do you do?"])?;
        npc(
            host,
            n,
            &[
                "We will look after your items and money for you. Leave your valuables with us if you want to keep them safe.",
            ],
        )?;
        c.goto("main options");

        c.label(&["jade's employment history"]);
        npc(
            host,
            ChatAnim::Friendly,
            &["Oh, ever since the Guild opened. I like it here."],
        )?;
        player(host, ChatAnim::Asking, &["Why's that?"])?;
        npc(
            host,
            ChatAnim::Friendly,
            &[
                "Well... what with all these warriors around, there's not much chance of my bank being robbed, is there?",
            ],
        )?;
        c.goto("nowhere");

        c.label(&["is the bounty hunter banker afraid"]);
        npc(
            host,
            n,
            &[
                "While the Wilderness is quite a dangerous place, The Bank of Gielinor offers us - roving bankers - extraordinary benefits for our hard work in hazardous environments.",
            ],
        )?;
        npc(
            host,
            n,
            &[
                "This allows us to provide our services to customers regardless of their current whereabouts. Our desire to serve is stronger than our fear of the Wilderness.",
            ],
        )?;
        c.goto("nowhere");
        Ok(())
    }
}
