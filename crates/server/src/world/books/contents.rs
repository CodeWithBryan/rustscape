//! The five global books' titles and pages, ported by oracle/books/port_contents.py from the unchanged
//! GeneralRuleBook, GoblinBook, GrimDiary, SecurityBookPlugin and StrongholdNotes. Do not edit by hand.
use super::{Line, Seg};

/// GeneralRuleBook.TITLE.
pub const GENERAL_RULES_TITLE: &[Seg] = &[Seg::Settings, Seg::T(" Rules")];
/// GeneralRuleBook.CONTENTS: page sets, pages, lines (message, child).
pub const GENERAL_RULES: &[&[&[Line]]] = &[
    &[
        &[
            Line(&[Seg::T("Rules")], 38),
            Line(&[Seg::T("1 - Offensive Language")], 104),
            Line(&[Seg::T("2 - Item Scamming")], 106),
            Line(&[Seg::T("3 - Password Scamming")], 108),
            Line(&[Seg::T("4 - Cheating/Bug Abuse")], 110),
            Line(&[Seg::T("5 - Staff Impersonation")], 112),
            Line(&[Seg::T("6 - Account Sharing/Trading")], 114),
            Line(&[Seg::T("7 - Using Third Party")], 116),
            Line(&[Seg::T("Software")], 118),
            Line(&[Seg::T("8 - Multi Logging-in")], 120),
            Line(&[Seg::T("9 - Encouraging Others to")], 122),
            Line(&[Seg::T("Break Rules")], 124),
            Line(&[Seg::T("10 - False Representation")], 126),
            Line(&[Seg::T("11 - Website Advertising")], 128),
        ],
        &[
            Line(&[Seg::T("12 - Read World Item")], 134),
            Line(&[Seg::T("Trading")], 136),
            Line(&[Seg::T("13 - Asking For Personal")], 138),
            Line(&[Seg::T("Details")], 140),
            Line(&[Seg::T("14 - Misuse of Official")], 142),
            Line(&[Seg::T("Forums")], 144),
            Line(&[Seg::T("15 - Advert Blocking")], 146),
        ],
    ],
    &[
        &[
            Line(&[Seg::T("<col=08088A>1 - Offensive Language")], 38),
            Line(&[Seg::T("You must not use any")], 40),
            Line(&[Seg::T("language which is offensive,")], 41),
            Line(&[Seg::T("racist or obscene. Remember,")], 42),
            Line(&[Seg::T("it's nice to be nice!")], 43),
        ],
        &[
            Line(&[Seg::T("<col=08088A>2 - Item Scamming")], 53),
            Line(&[Seg::T("You must not scam or")], 55),
            Line(&[Seg::T("deceive other players. This")], 56),
            Line(&[Seg::T("includes claiming that items")], 57),
            Line(&[Seg::T("are rare when they are not,")], 58),
            Line(&[Seg::T("team scamming or telling")], 59),
            Line(&[Seg::T("players that you can")], 60),
            Line(&[Seg::T("upgrade' their armour in")], 61),
            Line(&[Seg::T("any way!")], 62),
        ],
    ],
    &[
        &[
            Line(&[Seg::T("<col=08088A>3 - Password Scamming")], 38),
            Line(&[Seg::T("Asking for - or trying to")], 40),
            Line(&[Seg::T("obtain - another player's")], 41),
            Line(&[Seg::T("password in any way will not")], 42),
            Line(&[Seg::T("be tolerated, even in jest!")], 43),
        ],
        &[
            Line(&[Seg::T("<col=08088A>4 - Cheating/Bug Abuse")], 53),
            Line(&[Seg::T("A bug is a technical glitch")], 55),
            Line(&[Seg::T("found in the game. You")], 56),
            Line(&[Seg::T("must not use or attempt to")], 57),
            Line(&[Seg::T("use any cheats or errors that")], 58),
            Line(&[Seg::T("you find in our software.")], 59),
            Line(&[Seg::T("Any bugs found must be")], 60),
            Line(&[Seg::T("reported to "), Seg::Settings], 61),
            Line(&[Seg::T("immediately, by clicking the")], 62),
            Line(&[Seg::T("'Report a bug/fault' link")], 63),
            Line(&[Seg::T("found on the main page.")], 64),
        ],
    ],
    &[
        &[
            Line(&[Seg::T("<col=08088A>5 - Staff Impersonation")], 38),
            Line(&[Seg::T("You should not attempt to")], 40),
            Line(
                &[Seg::T("impersonate "), Seg::Settings, Seg::T(" staff in")],
                41,
            ),
            Line(&[Seg::T("any way.")], 42),
        ],
        &[
            Line(&[Seg::T("<col=08088A>6 - Account Sharing/Trading")], 53),
            Line(&[Seg::T("Each account should only be")], 55),
            Line(&[Seg::T("used by ONE person and")], 56),
            Line(&[Seg::T("ONE person alone.")], 57),
            Line(&[Seg::T("Remember that trying to")], 58),
            Line(&[Seg::T("buy, sell, borrow or give")], 59),
            Line(&[Seg::T("away an account is against")], 60),
            Line(&[Seg::T("the rules and will land you in")], 61),
            Line(&[Seg::T("trouble when caught!")], 62),
        ],
    ],
    &[
        &[Line(
            &[Seg::T(
                "<col=08088A>7 - Using Third Party <br><col=08088A>Software<br><br>You must not use other<br>programs to gain an unfair<br>advantage in the game.",
            )],
            38,
        )],
        &[Line(
            &[
                Seg::T(
                    "<col=08088A>8 - Multi Logging-in</col><br><br>If you create more than one<br>",
                ),
                Seg::Settings,
                Seg::T(
                    " account, they<br>must not interact. This<br>includes giving items to a<br>friend to transfer to another<br>account you own.",
                ),
            ],
            53,
        )],
    ],
    &[&[
        Line(
            &[
                Seg::T(
                    "<col=08088A>9 - Encouraging Others to<br><col=08088A>Break Rules</col><br><br>You must not encourage<br>others to break any of the<br>",
                ),
                Seg::Settings,
                Seg::T(" rules."),
            ],
            38,
        ),
        Line(
            &[
                Seg::T(
                    "<col=08088A>10 - False Representation</col><br><br>You must not misuse<br>",
                ),
                Seg::Settings,
                Seg::T(
                    " Customer<br>Support.Trying to get<br>another player into trouble<br>by reporting them for no<br>reason or framing them is an<br>abuse of the Customer<br>Support Team's service.<br> want to help as many<br>players as possible and so the<br>Customer Support Team's<br>service must be used<br>appropriately and treated with",
                ),
            ],
            53,
        ),
    ]],
    &[
        &[
            Line(&[Seg::T("respect at all times.")], 38),
            Line(
                &[
                    Seg::T(
                        "<col=08088A>11 - Website Advertising</col><br><br>You are not allowed to<br>actively advertise any<br>websites, fan-sites, IRC<br>channels or product<br>anywhere in ",
                    ),
                    Seg::Settings,
                    Seg::T(",<br>including the "),
                    Seg::Settings,
                    Seg::T("<br>forums."),
                ],
                53,
            ),
        ],
        &[],
    ],
    &[&[
        Line(
            &[
                Seg::T("<col=08088A>12 - Real World Item<br><col=08088A>Trading<br><br></col>"),
                Seg::Settings,
                Seg::T(
                    " items must only<br>be exchanged for other items<br>or service within the game.<br>Buying or selling items<br>outside of the ",
                ),
                Seg::Settings,
                Seg::T("<br>environment is not in the<br>spirit of the game and is easy<br>for "),
                Seg::Settings,
                Seg::T(" to trace!"),
            ],
            38,
        ),
        Line(
            &[Seg::T(
                "<col=08088A>13 - Asking for Personal<br><col=08088A>Details<br><br></col>To protect player's safety<br>and privacy, you must not<br>ask for personal details. This<br>includes full names, phone<br>numbers, MSN, AIM, email<br>and home/school addresses!",
            )],
            53,
        ),
    ]],
    &[&[
        Line(
            &[Seg::T(
                "<col=08088A>14 - Misuse of Official<br><col=08088A>Forums</col><br><br>Forums must be used in<br>accordance with the Forum<br>Code of Conduct and treated<br>with respect at all times.",
            )],
            38,
        ),
        Line(
            &[
                Seg::T(
                    "<col=08088A>15 - Advert Blocking<br><br>Blocking the adverts in the<br>free to play version of<br>",
                ),
                Seg::Settings,
                Seg::T(" is against the<br>rules."),
            ],
            53,
        ),
    ]],
];

/// GoblinBook.TITLE.
pub const GOBLIN_TITLE: &[Seg] = &[Seg::T("The Book of the Big High War God")];
/// GoblinBook.CONTENTS: page sets, pages, lines (message, child).
pub const GOBLIN: &[&[&[Line]]] = &[
    &[
        &[
            Line(&[Seg::T("<col=8A0808>The Creation of the Goblins")], 97),
            Line(&[Seg::T("")], 68),
            Line(&[Seg::T("In beginning all gods have")], 69),
            Line(&[Seg::T("big war. Each god find army")], 70),
            Line(&[Seg::T("to fight for him.")], 71),
            Line(&[Seg::T("")], 72),
            Line(&[Seg::T("Big High War God not have")], 73),
            Line(&[Seg::T("army. Big High War God go")], 74),
            Line(&[Seg::T("to beardy-short-people and")], 75),
            Line(&[Seg::T("ask, Will you fight in my")], 76),
            Line(&[Seg::T("army? But beardy-short-")], 77),
            Line(&[Seg::T("people say, No, we fight for")], 78),
            Line(&[Seg::T("God of Shiny Light.")], 79),
            Line(&[Seg::T("")], 80),
            Line(&[Seg::T("Then Big High War God go")], 81),
        ],
        &[
            Line(&[Seg::T("to demons and ask, Will you")], 82),
            Line(&[Seg::T("fight in my army? But")], 83),
            Line(&[Seg::T("demons say, No, we fight for")], 84),
            Line(&[Seg::T("God of Dark Fire.")], 85),
            Line(&[Seg::T("")], 86),
            Line(&[Seg::T("Then Big High War God go")], 87),
            Line(&[Seg::T("to tall people with keen blades")], 88),
            Line(&[Seg::T("and say, Will you fight in")], 89),
            Line(&[Seg::T("my army? But tall people")], 90),
            Line(&[Seg::T("with keen blades say, No,")], 91),
            Line(&[Seg::T("some of us fight for God of")], 92),
            Line(&[Seg::T("Shiny Light and some of us")], 93),
            Line(&[Seg::T("fight for God of Dark Fire,")], 94),
            Line(&[Seg::T("but none of us fight for you!")], 95),
            Line(&[Seg::T("")], 96),
        ],
    ],
    &[
        &[
            Line(&[Seg::T("Big High War God very sad.")], 97),
            Line(&[Seg::T("He travel east and west,")], 68),
            Line(&[Seg::T("north and south, across land")], 69),
            Line(&[Seg::T("looking for army to fight for")], 70),
            Line(&[Seg::T("him.")], 71),
            Line(&[Seg::T("")], 72),
            Line(&[Seg::T("Then goblins say, We fight")], 73),
            Line(&[Seg::T("for you! At that time goblins")], 74),
            Line(&[Seg::T("very weak, very small, soft")], 75),
            Line(&[Seg::T("skin. Not like goblins today!")], 76),
            Line(&[Seg::T("But Big High War God say,")], 77),
            Line(&[Seg::T("I will make you my army.")], 78),
            Line(&[Seg::T("So Big High War God train")], 79),
            Line(&[Seg::T("goblins so they very strong.")], 80),
            Line(&[Seg::T("He give them good armour")], 81),
        ],
        &[
            Line(&[Seg::T("so they not be harmed. He")], 82),
            Line(&[Seg::T("make them strong in spirit so")], 83),
            Line(&[Seg::T("they not afraid of battle. He")], 84),
            Line(&[Seg::T("give them commanders so")], 85),
            Line(&[Seg::T("they know which way to go.")], 86),
            Line(&[Seg::T("He divide goblins into twelve")], 87),
            Line(&[Seg::T("tribes and send them into")], 88),
            Line(&[Seg::T("battle!")], 89),
            Line(&[Seg::T("")], 90),
            Line(&[Seg::T("Goblin armies fight very")], 91),
            Line(&[Seg::T("good! When other gods see")], 92),
            Line(&[Seg::T("this, they very jealous. So")], 93),
            Line(&[Seg::T("they say, We want goblins to")], 94),
            Line(&[Seg::T("fight for us too! So Big High")], 95),
            Line(&[Seg::T("War God take some tribes")], 96),
        ],
    ],
    &[
        &[
            Line(&[Seg::T("and sell them, one to God of")], 97),
            Line(&[Seg::T("Shiny Light and one to God")], 68),
            Line(&[Seg::T("of Dark Fire, and other")], 69),
            Line(&[Seg::T("tribes to other gods. But")], 70),
            Line(&[Seg::T("most still worship Big High")], 71),
            Line(&[Seg::T("War God who created them!")], 72),
            Line(&[Seg::T("")], 73),
            Line(&[Seg::T("")], 74),
            Line(&[Seg::T("")], 75),
            Line(&[Seg::T("")], 76),
            Line(&[Seg::T("")], 77),
            Line(&[Seg::T("")], 78),
            Line(&[Seg::T("")], 79),
            Line(&[Seg::T("")], 80),
            Line(&[Seg::T("")], 81),
        ],
        &[
            Line(&[Seg::T("<col=8A0808>The Several Commandments")], 82),
            Line(&[Seg::T("")], 83),
            Line(&[Seg::T("These are commands of Big")], 84),
            Line(&[Seg::T("High War God! Obey all")], 85),
            Line(&[Seg::T("commands all time or Big")], 86),
            Line(&[Seg::T("High War God kill you very")], 87),
            Line(&[Seg::T("bad!")], 88),
            Line(&[Seg::T("")], 89),
            Line(&[Seg::T("Always to slay enemies of")], 90),
            Line(&[Seg::T("Big High War God. Enemies")], 91),
            Line(&[Seg::T("must die!")], 92),
            Line(&[Seg::T("")], 93),
            Line(&[Seg::T("Not to run from battle.")], 94),
            Line(&[Seg::T("Cowards must die!")], 95),
            Line(&[Seg::T("")], 96),
        ],
    ],
    &[
        &[
            Line(&[Seg::T("Not to show mercy. Merciful")], 97),
            Line(&[Seg::T("must die!")], 68),
            Line(&[Seg::T("")], 69),
            Line(&[Seg::T("Not to doubt Big High War")], 70),
            Line(&[Seg::T("God. Doubters must die!")], 71),
            Line(&[Seg::T("")], 72),
            Line(&[Seg::T("Not to make own plans.")], 73),
            Line(&[Seg::T("Thinkers must die!")], 74),
            Line(&[Seg::T("")], 75),
            Line(&[Seg::T("")], 76),
            Line(&[Seg::T("")], 77),
            Line(&[Seg::T("")], 78),
            Line(&[Seg::T("")], 79),
            Line(&[Seg::T("")], 80),
            Line(&[Seg::T("")], 81),
        ],
        &[
            Line(&[Seg::T("<col=8A0808>The end of the war and the")], 82),
            Line(&[Seg::T("<col=8A0808>prophecy")], 83),
            Line(&[Seg::T("")], 84),
            Line(&[Seg::T("The war of gods last many")], 85),
            Line(&[Seg::T("lifetimes. Battle is glorious")], 86),
            Line(&[Seg::T("and many heroes live and")], 87),
            Line(&[Seg::T("die! Then all gods leave")], 88),
            Line(&[Seg::T("world, leave their armies")], 89),
            Line(&[Seg::T("behind. But goblins still")], 90),
            Line(&[Seg::T("soldiers, still fight! Goblins")], 91),
            Line(&[Seg::T("fight against tall people with")], 92),
            Line(&[Seg::T("keen blades. But tall people")], 93),
            Line(&[Seg::T("build cities with walls, they")], 94),
            Line(&[Seg::T("not want to fight. They not")], 95),
            Line(&[Seg::T("true soldiers like goblins! But")], 96),
        ],
    ],
    &[
        &[
            Line(&[Seg::T("now goblins not have enough")], 97),
            Line(&[Seg::T("to eat, and have no")], 68),
            Line(&[Seg::T("commanders to tell them who")], 69),
            Line(&[Seg::T("to fight. So goblin tribes fight")], 70),
            Line(&[Seg::T("one another.")], 71),
            Line(&[Seg::T("")], 72),
            Line(&[Seg::T("At last all goblin tribes have")], 73),
            Line(&[Seg::T("big battle on plain of mud.")], 74),
            Line(&[Seg::T("Battle last many days and")], 75),
            Line(&[Seg::T("many goblins die, battle is")], 76),
            Line(&[Seg::T("glorious! But goblin corpses")], 77),
            Line(&[Seg::T("cover the ground and it look")], 78),
            Line(&[Seg::T("like all die. ")], 79),
            Line(&[Seg::T("")], 80),
            Line(&[Seg::T("Then Hopespear of the")], 81),
        ],
        &[
            Line(&[Seg::T("Narogoshunn tribe have")], 82),
            Line(&[Seg::T("vision of Big High War God.")], 83),
            Line(&[Seg::T("In night while soldiers rest he")], 84),
            Line(&[Seg::T("call leaders of all tribes")], 85),
            Line(&[Seg::T("together to give them")], 86),
            Line(&[Seg::T("message.")], 87),
            Line(&[Seg::T("")], 88),
            Line(&[Seg::T("This is the word of Big High")], 89),
            Line(&[Seg::T("War God: Battle is indeed")], 90),
            Line(&[Seg::T("glorious and goblins are")], 91),
            Line(&[Seg::T("soldiers but if there too")], 92),
            Line(&[Seg::T("much battle all goblins die!")], 93),
            Line(&[Seg::T("No more must goblin fight")], 94),
            Line(&[Seg::T("against goblin or tribe against")], 95),
            Line(&[Seg::T("tribe. Goblins must find other")], 96),
        ],
    ],
    &[
        &[
            Line(&[Seg::T("enemies to fight, but not fight")], 97),
            Line(&[Seg::T("each other!")], 68),
            Line(&[Seg::T("")], 69),
            Line(&[Seg::T("And this is the word of Big")], 70),
            Line(&[Seg::T("High War God: Today I")], 71),
            Line(&[Seg::T("cannot lead you, but")], 72),
            Line(&[Seg::T("someday I will send a new")], 73),
            Line(&[Seg::T("Commander to lead you.")], 74),
            Line(&[Seg::T("Under new Commander")], 75),
            Line(&[Seg::T("goblins will conquer all of")], 76),
            Line(&[Seg::Server, Seg::T(", every race and")], 77),
            Line(&[Seg::T("every god! And then Big")], 78),
            Line(&[Seg::T("High War God will return")], 79),
            Line(&[Seg::T("and sit on throne of bronze")], 80),
            Line(&[Seg::T("and rule over all. War will")], 81),
        ],
        &[
            Line(&[Seg::T("end in victory and victory")], 82),
            Line(&[Seg::T("will last forever!")], 83),
            Line(&[Seg::T("")], 84),
            Line(&[Seg::T("So leaders of tribes stop")], 85),
            Line(&[Seg::T("battle. And on plain of mud")], 86),
            Line(&[Seg::T("all tribes build temple to Big")], 87),
            Line(&[Seg::T("High War God and offer")], 88),
            Line(&[Seg::T("sacrifices.")], 89),
            Line(&[Seg::T("")], 90),
            Line(&[Seg::T("")], 91),
            Line(&[Seg::T("")], 92),
            Line(&[Seg::T("")], 93),
            Line(&[Seg::T("")], 94),
            Line(&[Seg::T("")], 95),
            Line(&[Seg::T("")], 96),
        ],
    ],
];

/// GrimDiary.TITLE.
pub const GRIM_DIARY_TITLE: &[Seg] = &[Seg::T("Diary of Death")];
/// GrimDiary.CONTENTS: page sets, pages, lines (message, child).
pub const GRIM_DIARY: &[&[&[Line]]] = &[
    &[
        &[Line(
            &[Seg::T(
                "My Diary<br><br>    by Grim<br><br><col=8A0808>'''''''''''''''''''''''''''''''<br><col=8A0808>''''12th Bennath<br><col=8A0808>'''''''''''''''''''''''''''''''<br>I had such a busy day<br>dealing out death today<br>It's not as easy being grim.<br>Realised that Alfonse is<br>such a good servant. He<br><br><br><br><br><br>",
            )],
            97,
        )],
        &[Line(
            &[Seg::T(
                "seems to have the house<br>in full working order. I<br>shall have to congratulate<br>him tomorrow. Spent<br>some well-deserved time<br>sharpening my scythe -<br>Alfonse kindly reminded<br>me to put the <col=FF0000>sharpener<br></col>back in the <col=FF0000>cabinet</col>. Such<br>a good chap.",
            )],
            82,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "<br><br><col=8A0808>'''''''''''''''''''''''''''''''<br><col=8A0808>'''13th Bennath<br><col=8A0808>'''''''''''''''''''''''''''''''<br></col>Went to the Wilderness<br>today. Plenty of foolish<br>people surrendering their<br>lives to me without<br>thought. My back is<br>killing me from all the<br>standing around waiting.",
            )],
            97,
        )],
        &[Line(
            &[Seg::T(
                "Must get that seen to.<br>Got my <col=FF0000>robes</col> stained<br>from one victim. Simply<br>ruined! I decided to<br>throw them in the<br><col=FF0000>fireplace</col> - will light the<br>fire soon. Oh, and I must<br>remember to call mother.",
            )],
            82,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "<br><br><col=8A0808>'''''''''''''''''''''''''''''''<br><col=8A0808>'''14th Bennath<br><col=8A0808>'''''''''''''''''''''''''''''''<br></col>A tragic day. Alfonse and<br>I were in the garden<br>looking at the state of the<br>spider nest. I patted my<br>trusty servant on the<br>back in thanks for all his<br>hard work. Sadly, my<br>touch of death killed him",
            )],
            97,
        )],
        &[Line(&[Seg::T("instantly. Feel quite guilty.")], 82)],
    ],
    &[
        &[Line(
            &[Seg::T(
                "<br><br><col=8A0808>'''''''''''''''''''''''''''''''<br><col=8A0808>'''20th Bennath<br><col=8A0808>'''''''''''''''''''''''''''''''<br></col>House is getting into<br>quite a state without<br>Alfonse - things strewn all<br>over the place. Almost<br>trod on the eye of my<br>mentor, eww. I put the<br><col=FF0000>eye</col> back on the <col=FF0000>shelf</col> so<br>he can watch over me",
            )],
            97,
        )],
        &[Line(
            &[Seg::T(
                "and make sure I stay<br>true to his teachings.<br>Went into Varrock to<br>buy some new robes -<br>people kept running away<br>in fear, so it was difficult<br>to find a sale.",
            )],
            82,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "<br><br><col=8A0808>'''''''''''''''''''''''''''''''<br><col=8A0808>'''21st Bennath<br><col=8A0808>'''''''''''''''''''''''''''''''<br></col>Decided to spend a bit of<br>time tidying today - it<br>really isn't an easy job.<br>In my activities I found<br>my old <col=FF0000> 'Voice of Doom'<br></col>potion on the <col=FF0000>bookcase</col> -<br>perfect for giving people a<br>good scare. Oh, I do love",
            )],
            97,
        )],
        &[Line(
            &[Seg::T(
                "my job.<br><br><br><col=8A0808>'''''''''''''''''''''''''''''''<br><col=8A0808>'''22nd Bennath<br><col=8A0808>'''''''''''''''''''''''''''''''</col><br>Ordered a new servant<br>today from the agency<br>and got a 10% discount<br>for getting past the<br>1000th servant mark.",
            )],
            82,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "Woo hoo! The agency<br>sent me his <col=FF0000>Last Will and<br>Testement</col>. Shall have to<br><col=FF0000>sit on that</col> for a while.<br><br><col=8A0808>'''''''''''''''''''''''''''''''<br><col=8A0808>'''23rd Bennath<br><col=8A0808>'''''''''''''''''''''''''''''''<br></col>Aquired some bones<br>today. Muncher should<br>appreciate them as a treat",
            )],
            97,
        )],
        &[Line(
            &[Seg::T(
                "the next time he behaves.<br>The problem is there<br>aren't many barriers he<br>can't devour, so I decided<br>to lock the <col=FF0000>bones</col> up in<br>the <col=FF0000>chest</col>.<br><br><col=8A0808>'''''''''''''''''''''''''''''''<col=8A0808><br><col=8A0808>''''24th Bennath<br><col=8A0808>'''''''''''''''''''''''''''''''<br>My plan to make undead",
            )],
            82,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "fish is going quite well. I<br>managed to obtain a<br>resurrection <col=FF0000>hourglass</col><br>today,so have added that<br>to the <col=FF0000>fishtank </col>to finish<br>off the process. It's so<br>difficult to have pets when<br>everything you touch dies<br>horrifically. I remember<br>having a rabbit once that<br>exploded when I fed it a",
            )],
            97,
        )],
        &[Line(
            &[Seg::T(
                "carrot!<br><br><col=8A0808>'''''''''''''''''''''''''''''''<br><col=8A0808>'''25th Bennath<br><col=8A0808>'''''''''''''''''''''''''''''''<br></col>Got back home today to<br>find Muncher has run<br>around the house<br>creating havok - he even<br>ate the postman! I",
            )],
            82,
        )],
    ],
    &[&[Line(
        &[Seg::T(
            "scolded him. So hopefully<br>he won't do it again<br>anytime soon. All my<br>things are in such a<br>mess. I'm surely going to<br>have to find someone to<br>tidy things up before my<br>new servant arrives.<br>Don't want to seem<br>totally incapable of looking<br>after myself.",
        )],
        97,
    )]],
];

/// SecurityBookPlugin.TITLE.
pub const SECURITY_TITLE: &[Seg] = &[Seg::T(""), Seg::Settings, Seg::T(" Account Security")];
/// SecurityBookPlugin.CONTENTS: page sets, pages, lines (message, child).
pub const SECURITY: &[&[&[Line]]] = &[
    &[
        &[
            Line(&[Seg::T("Chapters")], 38),
            Line(&[Seg::T("<h1><col=08088A>Password Tips</col></h1>")], 40),
            Line(
                &[Seg::T("<h1><col=08088A>Recovery Questions</col></h1>")],
                41,
            ),
            Line(
                &[Seg::T("<h1><col=08088A>Other Security Tips</col></h1>")],
                42,
            ),
            Line(
                &[Seg::T("<h1><col=08088A>Stringhold of Security</col></h1>")],
                43,
            ),
        ],
        &[
            Line(&[Seg::T("<h1><col=08088A>Password Tips</col></h1>")], 53),
            Line(&[Seg::T("A good password should be")], 54),
            Line(&[Seg::T("easily remembered by")], 55),
            Line(&[Seg::T("yourself but not easily")], 56),
            Line(&[Seg::T("guessed by anyone else.")], 57),
            Line(&[Seg::T("Choose a password that has")], 59),
            Line(&[Seg::T("both letters and numbers in")], 60),
            Line(&[Seg::T("it for the best security but")], 61),
            Line(&[Seg::T("don't make it so hard that")], 62),
            Line(&[Seg::T("you'll forget it!")], 63),
            Line(&[Seg::T("Never write your password")], 65),
            Line(&[Seg::T("down or leave it in a text file")], 66),
            Line(&[Seg::T("on your computer, someone")], 67),
        ],
    ],
    &[
        &[
            Line(&[Seg::T("could find it easily!")], 38),
            Line(&[Seg::T("Never tell anyone your")], 40),
            Line(
                &[Seg::T("password in "), Seg::Settings, Seg::T(", not")],
                41,
            ),
            Line(&[Seg::T("even a Moderator of any")], 42),
            Line(&[Seg::T("kind.")], 43),
        ],
        &[
            Line(
                &[Seg::T("<h1><col=08088A>Recovery Questions</col></h1>")],
                53,
            ),
            Line(&[Seg::T("Ideally your recovery")], 54),
            Line(&[Seg::T("questions should be easily")], 55),
            Line(&[Seg::T("remembered by you but not")], 56),
            Line(&[Seg::T("guessable by anyone who")], 57),
            Line(&[Seg::T("may know you or given")], 58),
            Line(&[Seg::T("away in conversation. Choose")], 59),
            Line(&[Seg::T("things that do not change,")], 60),
            Line(&[Seg::T("like dates or names but don't")], 61),
            Line(&[Seg::T("choose obvious ones like your")], 62),
            Line(&[Seg::T("birthday and your sister or")], 63),
            Line(&[Seg::T("bother's name because lots")], 64),
            Line(&[Seg::T("of people will know that.")], 65),
        ],
    ],
    &[
        &[
            Line(
                &[Seg::T("<h1><col=08088A>Recovery Questions</col></h1>")],
                38,
            ),
            Line(&[Seg::T("Bear in mind that recovery")], 39),
            Line(&[Seg::T("questions will take 14 days to")], 40),
            Line(&[Seg::T("become active after you have")], 41),
            Line(&[Seg::T("applied for them to be")], 42),
            Line(&[Seg::T("changed. This is to protect")], 43),
            Line(&[Seg::T("your account from hijackers")], 44),
            Line(&[Seg::T("who may change them.")], 45),
            Line(&[Seg::T("Never give your password to")], 47),
            Line(&[Seg::T("ANYONE. This includes")], 48),
            Line(&[Seg::T("your friends, family, and")], 49),
            Line(&[Seg::T("moderators in game.")], 50),
            Line(&[Seg::T("Never leave your account")], 52),
        ],
        &[
            Line(&[Seg::T("logged on if you are away")], 53),
            Line(&[Seg::T("from the computer, it only")], 54),
            Line(&[Seg::T("takes 5 seconds to steal your")], 55),
            Line(&[Seg::T("account!")], 56),
        ],
    ],
    &[
        &[
            Line(
                &[Seg::T("<h1><col=08088A>Stronghold of Security</col></h1>")],
                38,
            ),
            Line(&[Seg::T("Location: The Stronghold of")], 39),
            Line(&[Seg::T("Security, as we call it, is")], 40),
            Line(&[Seg::T("located under the village filled")], 41),
            Line(&[Seg::T("with Barbarians. It was")], 42),
            Line(&[Seg::T("found after they moved their")], 43),
            Line(&[Seg::T("mining operations and a")], 44),
            Line(&[Seg::T("miner fell through. The")], 45),
            Line(&[Seg::T("Stronghold contains many")], 46),
            Line(&[Seg::T("challenges. Both for those")], 47),
            Line(&[Seg::T("who enjoy combat and those")], 48),
            Line(&[Seg::T("who enjoy challenges of the")], 49),
            Line(&[Seg::T("mind. This book will be very")], 50),
            Line(&[Seg::T("useful to you in your travels")], 51),
            Line(&[Seg::T("there.")], 52),
        ],
        &[
            Line(&[Seg::T("You can find the Stronghold")], 53),
            Line(&[Seg::T("of Security by looking for a")], 54),
            Line(&[Seg::T("hole in Barbarian Village.")], 55),
            Line(&[Seg::T("Be sure to take your combat")], 56),
            Line(&[Seg::T("equipment though!")], 57),
        ],
    ],
];

/// StrongholdNotes.TITLE.
pub const STRONGHOLD_NOTES_TITLE: &[Seg] = &[Seg::T("Stronghold of Security - Notes")];
/// StrongholdNotes.CONTENTS: page sets, pages, lines (message, child).
pub const STRONGHOLD_NOTES: &[&[&[Line]]] = &[
    &[
        &[Line(
            &[Seg::T(
                "Chapters<br><br><col=08088A>Description<br><col=08088A>Level 1<br><col=08088A>Level 2<br><col=08088A>Level 3<br><col=08088A>Level 4<br><col=08088A>Navigation<br><col=08088A>Diary<br><br><br><br><br><br><br><br><br><br>",
            )],
            38,
        )],
        &[Line(
            &[Seg::T(
                "<col=08088A>Description<br>This stronghold was<br>unearthed by a miner<br>prospecting for new ores<br>around the Barbarian Village.<br>After gathering some<br>equipment he ventured into<br>the maze of tunnels and was<br>missing for a long time. He<br>finally emerged along with<br>copious notes refarding the<br>new beasts and strange<br>experiences which had befallen<br>him. He also mentioned that<br>there was treasure to be had,",
            )],
            53,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "but no one has been able to<br>wring a word from him about<br>this, he simply flapped his<br>arms and slapped his head.<br>This book details his notes<br>and my diary of exploration.<br>I am exploring to see if I<br>can find out more...",
            )],
            38,
        )],
        &[Line(
            &[Seg::T(
                "<col=08088A>Level 1<br>As well as goblins, creatures<br>like a man but also like a cow<br>infest this place! I have never<br>seen anything like this before.<br>The area itself is reminiscent<br>of frontline castles, with<br>many walls, doors and<br>skeletons of dead enemies.<br>I'm sure I hear voices in my<br>head each time I pass<br>through the gates. I have<br>dubbed this level War as it<br>seems like an eternal<br>battleground. I found only",
            )],
            53,
        )],
    ],
    &[
        &[Line(&[Seg::T("one small peaceful area here.")], 38)],
        &[Line(
            &[Seg::T(
                "<col=08088A>Level 2<br>My supplies are running low<br>and I find myself in barren<br>passages with seemingly<br>endless malnourished beasts<br>attacking me, revenous for<br>food. Nothing appears to be<br>able to grow, many<br>adventurers have died<br>through lack of food and the<br>very air appears to suck<br>vitality from me. I've come to<br>call this place famine.",
            )],
            53,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "<col=08088A>Level 3<br>Just breathing in this place<br>makes me shudder at the<br>thought of what foul disease I<br>may contract. The walls and<br>floor ooze and pulsate like<br>something pox ridden. There<br>is a very strange beast whom<br>I narrowly escaped from. At<br>first I thought it to be a<br>cross between a cow and a<br>sheep, something<br>domesticated... but when it<br>looked up at me I was<br>overcome with weakness and",
            )],
            38,
        )],
        &[Line(
            &[Seg::T(
                "barely got way with my life!<br>Luckily I found a small place<br>where I could heal myself<br>and rest a while. I have<br>named this area pestilence for<br>it reeks with decay.",
            )],
            53,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "<col=08088A>Level 4<br>On my first escapade into<br>this place I was utterly<br>shocked. The adventurers<br>who had come before me<br>must have made up a tiny<br>proportion of the skeletons of<br>the dead. Nothing truely<br>alive exists here, even those<br>beings who do wander the<br>halls are not alive as such,<br>but they do know that I am<br>and I get the distinct<br>impression that were they to<br>have their way, I would not",
            )],
            38,
        )],
        &[Line(
            &[Seg::T(
                "be for long! Death is<br>everywhere and thus I shall<br>name this place. There is one<br>small place of life, which was<br>gladdening to find and very<br>worth my while!",
            )],
            53,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "<col=08088A>Navigation<br>After getting lost several<br>times I finally worked out the<br>key to all the ladders and<br>chains around this death<br>infested place. All ropes and<br>chains will take you to the<br>start of the level that you are<br>on. However most ladders will<br>simply take you to the level<br>above. The one esception is<br>the ladder in the bottom level<br>treasure room, which appears<br>to lead through several<br>extremely twisty passages.",
            )],
            38,
        )],
        &[Line(
            &[Seg::T(
                "and eventually takes you out<br>of the dungeon completely.<br>The portals may be used if<br>you are of sufficient level or<br>have already claimed your<br>reward from the treasure<br>room.",
            )],
            38,
        )],
    ],
    &[
        &[Line(
            &[Seg::T(
                "<col=08088A>Diary<br>Day 1<br>Today I set out to find out<br>more about this place. From<br>my research I knew about<br>the sentient doors, imbued by<br>some unkown force to talk<br>to you and ask questions<br>before they will let you pass.<br>I have so far passed these<br>doors without incident, giving<br>the correct answer seems to<br>work a treat.<br><br>Day 2",
            )],
            38,
        )],
        &[Line(
            &[Seg::T(
                "I have fought my way<br>through the fearsome beasts<br>on the first level and am<br>preparing myself to journey<br>deeper. I hope that things are<br>not too difficult further on as<br>I am already sick of bread<br>and cheese for dinner.<br><br>Day 3<br>I ventured down into the<br>famine level today... I was<br>wounded and have returned<br>to the relative safety of the<br>level above. I am going to",
            )],
            53,
        )],
    ],
    &[&[Line(
        &[Seg::T(
            "try to make my way out<br>through the goblins and<br>mancow things... I hope I<br>make it.....",
        )],
        38,
    )]],
];
