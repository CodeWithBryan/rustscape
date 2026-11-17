//! FacialExpression: the chat-head animation each dialogue line plays. The
//! table is the original enum in declaration order; several familiar entries
//! share an animation id with a werewolf entry, as in the reference.
macro_rules! expressions {
    ($($variant:ident = ($name:literal, $id:literal),)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum FacialExpression {
            $($variant,)*
        }
        impl FacialExpression {
            /// Original `values()` order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];
            pub fn animation_id(self) -> i32 {
                match self {
                    $(Self::$variant => $id,)*
                }
            }
            /// Original enum constant name.
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }
        }
    };
}
expressions! {
    OldHappy = ("OLD_HAPPY", 588),
    OldCalmTalk1 = ("OLD_CALM_TALK1", 589),
    OldCalmTalk2 = ("OLD_CALM_TALK2", 590),
    OldDefault = ("OLD_DEFAULT", 591),
    OldEvil1 = ("OLD_EVIL1", 592),
    OldEvil2 = ("OLD_EVIL2", 593),
    OldNormal = ("OLD_NORMAL", 594),
    OldSneaky = ("OLD_SNEAKY", 595),
    OldDistressed = ("OLD_DISTRESSED", 596),
    OldDistressed2 = ("OLD_DISTRESSED2", 597),
    OldAlmostCrying = ("OLD_ALMOST_CRYING", 598),
    OldBowsHeadSad = ("OLD_BOWS_HEAD_SAD", 599),
    OldDrunkLeft = ("OLD_DRUNK_LEFT", 600),
    OldDrunkRight = ("OLD_DRUNK_RIGHT", 601),
    OldNotInterested = ("OLD_NOT_INTERESTED", 602),
    OldSleepy = ("OLD_SLEEPY", 603),
    OldPlainEvil = ("OLD_PLAIN_EVIL", 604),
    OldLaugh1 = ("OLD_LAUGH1", 605),
    OldLaugh2 = ("OLD_LAUGH2", 606),
    OldLaugh3 = ("OLD_LAUGH3", 607),
    OldLaugh4 = ("OLD_LAUGH4", 608),
    OldEvilLaugh = ("OLD_EVIL_LAUGH", 609),
    OldSad = ("OLD_SAD", 610),
    OldMoreSad = ("OLD_MORE_SAD", 611),
    OldOnOneHand = ("OLD_ON_ONE_HAND", 612),
    OldNearlyCrying = ("OLD_NEARLY_CRYING", 613),
    OldAngry1 = ("OLD_ANGRY1", 614),
    OldAngry2 = ("OLD_ANGRY2", 615),
    OldAngry3 = ("OLD_ANGRY3", 616),
    OldAngry4 = ("OLD_ANGRY4", 617),
    NodYes = ("NOD_YES", 9741),
    Worried = ("WORRIED", 9743),
    HalfWorried = ("HALF_WORRIED", 9745),
    Amazed = ("AMAZED", 9746),
    AmazedTalking = ("AMAZED_TALKING", 9749),
    ExtremelyShocked = ("EXTREMELY_SHOCKED", 9750),
    Guilty = ("GUILTY", 9758),
    HalfGuilty = ("HALF_GUILTY", 9760),
    Sad = ("SAD", 9761),
    SadTalking = ("SAD_TALKING", 9764),
    Crying = ("CRYING", 9765),
    HalfCrying = ("HALF_CRYING", 9768),
    Afraid = ("AFRAID", 9772),
    Scared = ("SCARED", 9776),
    Panicked = ("PANICKED", 9780),
    Annoyed = ("ANNOYED", 9784),
    Angry = ("ANGRY", 9785),
    Furious = ("FURIOUS", 9792),
    AngryWithSmile = ("ANGRY_WITH_SMILE", 9796),
    AngryWithSmileAndEvilEye = ("ANGRY_WITH_SMILE_AND_EVIL_EYE", 9798),
    Sleeping = ("SLEEPING", 9802),
    Silent = ("SILENT", 9804),
    Neutral = ("NEUTRAL", 9808),
    NeutralFast = ("NEUTRAL_FAST", 9810),
    Thinking = ("THINKING", 9812),
    HalfThinking = ("HALF_THINKING", 9814),
    Disgusted = ("DISGUSTED", 9816),
    DisgustedHeadShake = ("DISGUSTED_HEAD_SHAKE", 9823),
    Asking = ("ASKING", 9827),
    HalfAsking = ("HALF_ASKING", 9830),
    RollingEyes = ("ROLLING_EYES", 9831),
    HalfRollingEyes = ("HALF_ROLLING_EYES", 9834),
    Drunk = ("DRUNK", 9835),
    Suspicious = ("SUSPICIOUS", 9836),
    Laugh = ("LAUGH", 9840),
    LoudlyLaughing = ("LOUDLY_LAUGHING", 9841),
    EvilLaugh = ("EVIL_LAUGH", 9842),
    Friendly = ("FRIENDLY", 9844),
    Happy = ("HAPPY", 9847),
    Jolly = ("JOLLY", 9851),
    Struggle = ("STRUGGLE", 9865),
    WerewolfSad = ("WEREWOLF_SAD", 6550),
    WerewolfNeutral = ("WEREWOLF_NEUTRAL", 6551),
    WerewolfSuspicious = ("WEREWOLF_SUSPICIOUS", 6552),
    WerewolfThinking = ("WEREWOLF_THINKING", 6553),
    WerewolfHappy = ("WEREWOLF_HAPPY", 6555),
    ChildAngry = ("CHILD_ANGRY", 7168),
    ChildSideEye = ("CHILD_SIDE_EYE", 7169),
    ChildRecalling = ("CHILD_RECALLING", 7170),
    ChildEvilLaugh = ("CHILD_EVIL_LAUGH", 7171),
    ChildFriendly = ("CHILD_FRIENDLY", 7172),
    ChildNormal = ("CHILD_NORMAL", 7173),
    ChildNeutral = ("CHILD_NEUTRAL", 7174),
    ChildLoudlyLaughing = ("CHILD_LOUDLY_LAUGHING", 7175),
    ChildThinking = ("CHILD_THINKING", 7176),
    ChildSad = ("CHILD_SAD", 7177),
    ChildGuilty = ("CHILD_GUILTY", 7178),
    ChildSuspicious = ("CHILD_SUSPICIOUS", 7179),
    ChildSurprised = ("CHILD_SURPRISED", 7180),
    GraahkNod = ("GRAAHK_NOD", 6551),
    GhraakShakeMild = ("GHRAAK_SHAKE_MILD", 6553),
    GhraakShakeVigorous = ("GHRAAK_SHAKE_VIGOROUS", 6555),
}
impl FacialExpression {
    /// Original `valueOf`: exact, case-sensitive constant name.
    pub fn value_of(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|e| e.name() == name)
    }
    /// The `expression == null ? -1 : expression.getAnimationId()` conversion
    /// every FacialExpression overload of sendDialogues performs.
    pub fn or_unset(expression: Option<Self>) -> i32 {
        expression.map_or(-1, Self::animation_id)
    }
}
