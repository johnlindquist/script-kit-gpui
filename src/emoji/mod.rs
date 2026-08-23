//! Emoji data, search, and picker for the built-in emoji surface.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emoji {
    pub emoji: &'static str,
    pub name: &'static str,
    pub keywords: &'static [&'static str],
    pub category: EmojiCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIter)]
pub enum EmojiCategory {
    SmileysEmotion,
    PeopleBody,
    AnimalsNature,
    FoodDrink,
    TravelPlaces,
    Activities,
    Objects,
    Symbols,
    Flags,
}

impl EmojiCategory {
    pub const fn display_name(self) -> &'static str {
        match self {
            EmojiCategory::SmileysEmotion => "Smileys & Emotion",
            EmojiCategory::PeopleBody => "People & Body",
            EmojiCategory::AnimalsNature => "Animals & Nature",
            EmojiCategory::FoodDrink => "Food & Drink",
            EmojiCategory::TravelPlaces => "Travel & Places",
            EmojiCategory::Activities => "Activities",
            EmojiCategory::Objects => "Objects",
            EmojiCategory::Symbols => "Symbols",
            EmojiCategory::Flags => "Flags",
        }
    }
}

macro_rules! emoji {
    ($emoji:expr, $name:expr, $category:expr, [$($keyword:expr),+ $(,)?]) => {
        Emoji {
            emoji: $emoji,
            name: $name,
            keywords: &[$($keyword),+],
            category: $category,
        }
    };
}

use strum::IntoEnumIterator;
use EmojiCategory::*;

/// Returns an iterator over all emoji categories in declaration order.
pub fn all_categories() -> impl Iterator<Item = EmojiCategory> {
    EmojiCategory::iter()
}

pub const EMOJIS: &[Emoji] = &[
    emoji!(
        "😀",
        "grinning face",
        SmileysEmotion,
        ["happy", "smile", "face"]
    ),
    emoji!(
        "😃",
        "grinning face with big eyes",
        SmileysEmotion,
        ["happy", "joy", "eyes"]
    ),
    emoji!(
        "😄",
        "grinning face with smiling eyes",
        SmileysEmotion,
        ["smile", "eyes", "cheerful"]
    ),
    emoji!(
        "😁",
        "beaming face with smiling eyes",
        SmileysEmotion,
        ["beam", "smile", "teeth"]
    ),
    emoji!(
        "😆",
        "grinning squinting face",
        SmileysEmotion,
        ["laugh", "squint", "face"]
    ),
    emoji!(
        "😅",
        "grinning face with sweat",
        SmileysEmotion,
        ["relief", "sweat", "laugh"]
    ),
    emoji!(
        "😂",
        "face with tears of joy",
        SmileysEmotion,
        ["laugh", "tears", "funny"]
    ),
    emoji!(
        "🤣",
        "rolling on the floor laughing",
        SmileysEmotion,
        ["rofl", "laugh", "hilarious"]
    ),
    emoji!(
        "😊",
        "smiling face with smiling eyes",
        SmileysEmotion,
        ["blush", "smile", "warm"]
    ),
    emoji!(
        "😇",
        "smiling face with halo",
        SmileysEmotion,
        ["angel", "innocent", "halo"]
    ),
    emoji!(
        "🙂",
        "slightly smiling face",
        SmileysEmotion,
        ["smile", "friendly", "calm"]
    ),
    emoji!(
        "🙃",
        "upside-down face",
        SmileysEmotion,
        ["silly", "sarcasm", "playful"]
    ),
    emoji!(
        "😉",
        "winking face",
        SmileysEmotion,
        ["wink", "flirt", "playful"]
    ),
    emoji!(
        "😌",
        "relieved face",
        SmileysEmotion,
        ["relaxed", "calm", "relief"]
    ),
    emoji!(
        "😍",
        "smiling face with heart-eyes",
        SmileysEmotion,
        ["love", "crush", "adore"]
    ),
    emoji!(
        "🥰",
        "smiling face with hearts",
        SmileysEmotion,
        ["love", "affection", "hearts"]
    ),
    emoji!(
        "😘",
        "face blowing a kiss",
        SmileysEmotion,
        ["kiss", "love", "flirty"]
    ),
    emoji!(
        "😗",
        "kissing face",
        SmileysEmotion,
        ["kiss", "smooch", "love"]
    ),
    emoji!(
        "😙",
        "kissing face with smiling eyes",
        SmileysEmotion,
        ["kiss", "smile", "affection"]
    ),
    emoji!(
        "😚",
        "kissing face with closed eyes",
        SmileysEmotion,
        ["kiss", "shy", "love"]
    ),
    emoji!(
        "😋",
        "face savoring food",
        SmileysEmotion,
        ["yum", "food", "delicious"]
    ),
    emoji!(
        "😛",
        "face with tongue",
        SmileysEmotion,
        ["tongue", "playful", "tease"]
    ),
    emoji!(
        "😜",
        "winking face with tongue",
        SmileysEmotion,
        ["wink", "tongue", "joke"]
    ),
    emoji!(
        "🤪",
        "zany face",
        SmileysEmotion,
        ["crazy", "wild", "silly"]
    ),
    emoji!(
        "😝",
        "squinting face with tongue",
        SmileysEmotion,
        ["tongue", "silly", "tease"]
    ),
    emoji!(
        "🤑",
        "money-mouth face",
        SmileysEmotion,
        ["money", "rich", "cash"]
    ),
    emoji!(
        "🤗",
        "hugging face",
        SmileysEmotion,
        ["hug", "warm", "care"]
    ),
    emoji!(
        "🤭",
        "face with hand over mouth",
        SmileysEmotion,
        ["oops", "giggle", "surprised"]
    ),
    emoji!(
        "🤫",
        "shushing face",
        SmileysEmotion,
        ["quiet", "secret", "shh"]
    ),
    emoji!(
        "🤔",
        "thinking face",
        SmileysEmotion,
        ["think", "hmm", "question"]
    ),
    emoji!(
        "🤐",
        "zipper-mouth face",
        SmileysEmotion,
        ["silent", "secret", "zip"]
    ),
    emoji!(
        "🤨",
        "face with raised eyebrow",
        SmileysEmotion,
        ["skeptical", "doubt", "hmm"]
    ),
    emoji!(
        "😐",
        "neutral face",
        SmileysEmotion,
        ["meh", "neutral", "flat"]
    ),
    emoji!(
        "😑",
        "expressionless face",
        SmileysEmotion,
        ["blank", "deadpan", "neutral"]
    ),
    emoji!(
        "😶",
        "face without mouth",
        SmileysEmotion,
        ["speechless", "silent", "quiet"]
    ),
    emoji!(
        "😏",
        "smirking face",
        SmileysEmotion,
        ["smirk", "flirt", "sly"]
    ),
    emoji!(
        "😒",
        "unamused face",
        SmileysEmotion,
        ["annoyed", "meh", "sideeye"]
    ),
    emoji!(
        "🙄",
        "face with rolling eyes",
        SmileysEmotion,
        ["eyeroll", "annoyed", "sarcasm"]
    ),
    emoji!(
        "😬",
        "grimacing face",
        SmileysEmotion,
        ["awkward", "oops", "tense"]
    ),
    emoji!(
        "😮",
        "face with open mouth",
        SmileysEmotion,
        ["wow", "surprised", "shock"]
    ),
    emoji!(
        "😯",
        "hushed face",
        SmileysEmotion,
        ["quiet", "surprised", "wow"]
    ),
    emoji!(
        "😲",
        "astonished face",
        SmileysEmotion,
        ["astonished", "surprised", "amazed"]
    ),
    emoji!(
        "😳",
        "flushed face",
        SmileysEmotion,
        ["embarrassed", "blush", "shy"]
    ),
    emoji!(
        "🥺",
        "pleading face",
        SmileysEmotion,
        ["please", "puppy", "beg"]
    ),
    emoji!(
        "😢",
        "crying face",
        SmileysEmotion,
        ["sad", "tear", "upset"]
    ),
    emoji!(
        "😭",
        "loudly crying face",
        SmileysEmotion,
        ["cry", "sob", "sad"]
    ),
    emoji!(
        "😤",
        "face with steam from nose",
        SmileysEmotion,
        ["frustrated", "triumph", "huff"]
    ),
    emoji!(
        "😠",
        "angry face",
        SmileysEmotion,
        ["angry", "mad", "upset"]
    ),
    emoji!(
        "😡",
        "pouting face",
        SmileysEmotion,
        ["rage", "angry", "mad"]
    ),
    emoji!(
        "🤬",
        "face with symbols on mouth",
        SmileysEmotion,
        ["swear", "cursing", "rage"]
    ),
    emoji!(
        "😱",
        "face screaming in fear",
        SmileysEmotion,
        ["scared", "shock", "scream"]
    ),
    emoji!(
        "😨",
        "fearful face",
        SmileysEmotion,
        ["fear", "anxious", "scared"]
    ),
    emoji!(
        "😰",
        "anxious face with sweat",
        SmileysEmotion,
        ["stress", "anxious", "sweat"]
    ),
    emoji!(
        "😥",
        "sad but relieved face",
        SmileysEmotion,
        ["relief", "sad", "whew"]
    ),
    emoji!(
        "😓",
        "downcast face with sweat",
        SmileysEmotion,
        ["tired", "sweat", "sad"]
    ),
    emoji!(
        "🤯",
        "exploding head",
        SmileysEmotion,
        ["mindblown", "shock", "wow"]
    ),
    emoji!(
        "😴",
        "sleeping face",
        SmileysEmotion,
        ["sleep", "tired", "zzz"]
    ),
    emoji!(
        "🤤",
        "drooling face",
        SmileysEmotion,
        ["drool", "hungry", "desire"]
    ),
    emoji!(
        "😪",
        "sleepy face",
        SmileysEmotion,
        ["sleepy", "drowsy", "tired"]
    ),
    emoji!(
        "🤢",
        "nauseated face",
        SmileysEmotion,
        ["sick", "nausea", "gross"]
    ),
    emoji!(
        "🤮",
        "face vomiting",
        SmileysEmotion,
        ["vomit", "sick", "ill"]
    ),
    emoji!(
        "🤧",
        "sneezing face",
        SmileysEmotion,
        ["sneeze", "sick", "cold"]
    ),
    emoji!("🥵", "hot face", SmileysEmotion, ["hot", "sweat", "heat"]),
    emoji!(
        "🥶",
        "cold face",
        SmileysEmotion,
        ["cold", "freezing", "chill"]
    ),
    emoji!(
        "😵",
        "dizzy face",
        SmileysEmotion,
        ["dizzy", "confused", "woozy"]
    ),
    emoji!(
        "🥴",
        "woozy face",
        SmileysEmotion,
        ["woozy", "drunk", "dizzy"]
    ),
    emoji!(
        "😎",
        "smiling face with sunglasses",
        SmileysEmotion,
        ["cool", "sunglasses", "chill"]
    ),
    emoji!("🤓", "nerd face", SmileysEmotion, ["nerd", "smart", "geek"]),
    emoji!(
        "🧐",
        "face with monocle",
        SmileysEmotion,
        ["inspect", "fancy", "curious"]
    ),
    emoji!(
        "🤠",
        "cowboy hat face",
        SmileysEmotion,
        ["cowboy", "western", "hat"]
    ),
    emoji!(
        "🥳",
        "partying face",
        SmileysEmotion,
        ["party", "celebrate", "birthday"]
    ),
    emoji!(
        "😈",
        "smiling face with horns",
        SmileysEmotion,
        ["devil", "mischief", "horns"]
    ),
    emoji!(
        "👿",
        "angry face with horns",
        SmileysEmotion,
        ["devil", "angry", "horns"]
    ),
    emoji!("💀", "skull", SmileysEmotion, ["dead", "skull", "spooky"]),
    emoji!(
        "☠️",
        "skull and crossbones",
        SmileysEmotion,
        ["danger", "pirate", "poison"]
    ),
    emoji!(
        "💩",
        "pile of poo",
        SmileysEmotion,
        ["poo", "funny", "toilet"]
    ),
    emoji!(
        "🤡",
        "clown face",
        SmileysEmotion,
        ["clown", "circus", "silly"]
    ),
    emoji!(
        "👻",
        "ghost",
        SmileysEmotion,
        ["ghost", "spooky", "halloween"]
    ),
    emoji!("👽", "alien", SmileysEmotion, ["alien", "ufo", "space"]),
    emoji!("🤖", "robot", SmileysEmotion, ["robot", "ai", "bot"]),
    emoji!(
        "😺",
        "grinning cat",
        SmileysEmotion,
        ["cat", "smile", "pet"]
    ),
    emoji!(
        "😸",
        "grinning cat with smiling eyes",
        SmileysEmotion,
        ["cat", "joy", "pet"]
    ),
    emoji!(
        "😹",
        "cat with tears of joy",
        SmileysEmotion,
        ["cat", "laugh", "tears"]
    ),
    emoji!(
        "😻",
        "smiling cat with heart-eyes",
        SmileysEmotion,
        ["cat", "love", "heart"]
    ),
    emoji!(
        "😼",
        "cat with wry smile",
        SmileysEmotion,
        ["cat", "smirk", "pet"]
    ),
    emoji!("😽", "kissing cat", SmileysEmotion, ["cat", "kiss", "love"]),
    emoji!(
        "🙀",
        "weary cat",
        SmileysEmotion,
        ["cat", "shock", "scared"]
    ),
    emoji!("😿", "crying cat", SmileysEmotion, ["cat", "sad", "tears"]),
    emoji!("😾", "pouting cat", SmileysEmotion, ["cat", "angry", "mad"]),
    emoji!("❤️", "red heart", Symbols, ["heart", "love", "red"]),
    emoji!("🧡", "orange heart", Symbols, ["heart", "love", "orange"]),
    emoji!("💛", "yellow heart", Symbols, ["heart", "love", "yellow"]),
    emoji!("💚", "green heart", Symbols, ["heart", "love", "green"]),
    emoji!("💙", "blue heart", Symbols, ["heart", "love", "blue"]),
    emoji!("💜", "purple heart", Symbols, ["heart", "love", "purple"]),
    emoji!("🖤", "black heart", Symbols, ["heart", "love", "black"]),
    emoji!("🤍", "white heart", Symbols, ["heart", "love", "white"]),
    emoji!("🤎", "brown heart", Symbols, ["heart", "love", "brown"]),
    emoji!(
        "💔",
        "broken heart",
        Symbols,
        ["heartbreak", "sad", "breakup"]
    ),
    emoji!(
        "❣️",
        "heart exclamation",
        Symbols,
        ["heart", "emphasis", "love"]
    ),
    emoji!("💕", "two hearts", Symbols, ["hearts", "love", "affection"]),
    emoji!(
        "💞",
        "revolving hearts",
        Symbols,
        ["hearts", "romance", "love"]
    ),
    emoji!("💓", "beating heart", Symbols, ["heart", "pulse", "love"]),
    emoji!("💗", "growing heart", Symbols, ["heart", "love", "grow"]),
    emoji!(
        "💖",
        "sparkling heart",
        Symbols,
        ["heart", "sparkle", "love"]
    ),
    emoji!(
        "💘",
        "heart with arrow",
        Symbols,
        ["cupid", "heart", "romance"]
    ),
    emoji!(
        "💝",
        "heart with ribbon",
        Symbols,
        ["gift", "heart", "love"]
    ),
    emoji!("💯", "hundred points", Symbols, ["100", "perfect", "score"]),
    emoji!("💥", "collision", Symbols, ["boom", "impact", "explode"]),
    emoji!("💫", "dizzy", Symbols, ["dizzy", "star", "sparkle"]),
    emoji!("💦", "sweat droplets", Symbols, ["sweat", "water", "drops"]),
    emoji!("💨", "dashing away", Symbols, ["speed", "dash", "wind"]),
    emoji!(
        "👋",
        "waving hand",
        PeopleBody,
        ["wave", "hello", "goodbye"]
    ),
    emoji!(
        "🤚",
        "raised back of hand",
        PeopleBody,
        ["hand", "raised", "stop"]
    ),
    emoji!(
        "🖐️",
        "hand with fingers splayed",
        PeopleBody,
        ["hand", "five", "palm"]
    ),
    emoji!(
        "✋",
        "raised hand",
        PeopleBody,
        ["hand", "stop", "highfive"]
    ),
    emoji!(
        "🖖",
        "vulcan salute",
        PeopleBody,
        ["vulcan", "spock", "salute"]
    ),
    emoji!("👌", "ok hand", PeopleBody, ["ok", "hand", "perfect"]),
    emoji!(
        "🤌",
        "pinched fingers",
        PeopleBody,
        ["gesture", "pinched", "italian"]
    ),
    emoji!(
        "🤏",
        "pinching hand",
        PeopleBody,
        ["small", "pinch", "tiny"]
    ),
    emoji!(
        "✌️",
        "victory hand",
        PeopleBody,
        ["peace", "victory", "hand"]
    ),
    emoji!(
        "🤞",
        "crossed fingers",
        PeopleBody,
        ["luck", "hope", "fingers"]
    ),
    emoji!(
        "🫰",
        "hand with index finger and thumb crossed",
        PeopleBody,
        ["heart", "finger", "gesture"]
    ),
    emoji!(
        "🤟",
        "love-you gesture",
        PeopleBody,
        ["ily", "hand", "love"]
    ),
    emoji!(
        "🤘",
        "sign of the horns",
        PeopleBody,
        ["rock", "metal", "hand"]
    ),
    emoji!("🤙", "call me hand", PeopleBody, ["call", "phone", "shaka"]),
    emoji!(
        "👈",
        "backhand index pointing left",
        PeopleBody,
        ["left", "point", "hand"]
    ),
    emoji!(
        "👉",
        "backhand index pointing right",
        PeopleBody,
        ["right", "point", "hand"]
    ),
    emoji!(
        "👆",
        "backhand index pointing up",
        PeopleBody,
        ["up", "point", "hand"]
    ),
    emoji!(
        "👇",
        "backhand index pointing down",
        PeopleBody,
        ["down", "point", "hand"]
    ),
    emoji!(
        "☝️",
        "index pointing up",
        PeopleBody,
        ["up", "index", "point"]
    ),
    emoji!("👍", "thumbs up", PeopleBody, ["approve", "like", "yes"]),
    emoji!("👎", "thumbs down", PeopleBody, ["dislike", "no", "reject"]),
    emoji!("👊", "oncoming fist", PeopleBody, ["fist", "punch", "bump"]),
    emoji!(
        "✊",
        "raised fist",
        PeopleBody,
        ["fist", "power", "solidarity"]
    ),
    emoji!(
        "🤛",
        "left-facing fist",
        PeopleBody,
        ["fist", "left", "bump"]
    ),
    emoji!(
        "🤜",
        "right-facing fist",
        PeopleBody,
        ["fist", "right", "bump"]
    ),
    emoji!(
        "👏",
        "clapping hands",
        PeopleBody,
        ["clap", "applause", "praise"]
    ),
    emoji!(
        "🙌",
        "raising hands",
        PeopleBody,
        ["hooray", "celebrate", "hands"]
    ),
    emoji!("👐", "open hands", PeopleBody, ["open", "hug", "hands"]),
    emoji!(
        "🤲",
        "palms up together",
        PeopleBody,
        ["offer", "prayer", "hands"]
    ),
    emoji!(
        "🤝",
        "handshake",
        PeopleBody,
        ["deal", "agreement", "greet"]
    ),
    emoji!(
        "🙏",
        "folded hands",
        PeopleBody,
        ["please", "thanks", "pray"]
    ),
    emoji!(
        "💪",
        "flexed biceps",
        PeopleBody,
        ["strong", "muscle", "gym"]
    ),
    emoji!(
        "🦾",
        "mechanical arm",
        PeopleBody,
        ["prosthetic", "robot", "strength"]
    ),
    emoji!("🧠", "brain", PeopleBody, ["brain", "smart", "mind"]),
    emoji!("👀", "eyes", PeopleBody, ["look", "watch", "see"]),
    emoji!("👁️", "eye", PeopleBody, ["eye", "vision", "watch"]),
    emoji!("👄", "mouth", PeopleBody, ["lips", "mouth", "speak"]),
    emoji!("👅", "tongue", PeopleBody, ["tongue", "taste", "playful"]),
    emoji!("👂", "ear", PeopleBody, ["listen", "ear", "hear"]),
    emoji!("👃", "nose", PeopleBody, ["smell", "nose", "face"]),
    emoji!("🫶", "heart hands", PeopleBody, ["heart", "hands", "love"]),
    emoji!("🐶", "dog face", AnimalsNature, ["dog", "pet", "puppy"]),
    emoji!("🐱", "cat face", AnimalsNature, ["cat", "pet", "kitty"]),
    emoji!(
        "🐭",
        "mouse face",
        AnimalsNature,
        ["mouse", "animal", "small"]
    ),
    emoji!(
        "🐹",
        "hamster face",
        AnimalsNature,
        ["hamster", "pet", "cute"]
    ),
    emoji!(
        "🐰",
        "rabbit face",
        AnimalsNature,
        ["rabbit", "bunny", "cute"]
    ),
    emoji!("🦊", "fox", AnimalsNature, ["fox", "animal", "wild"]),
    emoji!("🐻", "bear", AnimalsNature, ["bear", "animal", "wild"]),
    emoji!("🐼", "panda", AnimalsNature, ["panda", "bear", "cute"]),
    emoji!(
        "🐨",
        "koala",
        AnimalsNature,
        ["koala", "animal", "australia"]
    ),
    emoji!("🐯", "tiger face", AnimalsNature, ["tiger", "cat", "wild"]),
    emoji!("🦁", "lion", AnimalsNature, ["lion", "cat", "wild"]),
    emoji!("🐮", "cow face", AnimalsNature, ["cow", "farm", "animal"]),
    emoji!("🐷", "pig face", AnimalsNature, ["pig", "farm", "animal"]),
    emoji!("🐸", "frog", AnimalsNature, ["frog", "animal", "green"]),
    emoji!(
        "🐵",
        "monkey face",
        AnimalsNature,
        ["monkey", "animal", "primate"]
    ),
    emoji!(
        "🦋",
        "butterfly",
        AnimalsNature,
        ["butterfly", "insect", "nature"]
    ),
    emoji!(
        "🌸",
        "cherry blossom",
        AnimalsNature,
        ["flower", "spring", "pink"]
    ),
    emoji!(
        "🌻",
        "sunflower",
        AnimalsNature,
        ["flower", "sun", "nature"]
    ),
    emoji!("🌈", "rainbow", AnimalsNature, ["rainbow", "color", "sky"]),
    emoji!(
        "🌙",
        "crescent moon",
        AnimalsNature,
        ["moon", "night", "sky"]
    ),
    emoji!("☀️", "sun", AnimalsNature, ["sun", "weather", "bright"]),
    emoji!("🔥", "fire", AnimalsNature, ["fire", "lit", "hot"]),
    emoji!("🍎", "red apple", FoodDrink, ["apple", "fruit", "food"]),
    emoji!("🍕", "pizza", FoodDrink, ["pizza", "food", "slice"]),
    emoji!("🍔", "hamburger", FoodDrink, ["burger", "food", "meal"]),
    emoji!("🍟", "french fries", FoodDrink, ["fries", "food", "snack"]),
    emoji!("🌮", "taco", FoodDrink, ["taco", "food", "mexican"]),
    emoji!("🍣", "sushi", FoodDrink, ["sushi", "food", "japanese"]),
    emoji!(
        "🍜",
        "steaming bowl",
        FoodDrink,
        ["ramen", "noodles", "soup"]
    ),
    emoji!("🍩", "doughnut", FoodDrink, ["donut", "sweet", "dessert"]),
    emoji!("🍪", "cookie", FoodDrink, ["cookie", "sweet", "dessert"]),
    emoji!("☕", "hot beverage", FoodDrink, ["coffee", "tea", "drink"]),
    emoji!("🍺", "beer mug", FoodDrink, ["beer", "drink", "bar"]),
    emoji!("🍷", "wine glass", FoodDrink, ["wine", "drink", "glass"]),
    emoji!("🥤", "cup with straw", FoodDrink, ["drink", "soda", "cold"]),
    emoji!("🧋", "bubble tea", FoodDrink, ["boba", "tea", "drink"]),
    emoji!("🍿", "popcorn", FoodDrink, ["snack", "movie", "popcorn"]),
    emoji!("📱", "mobile phone", Objects, ["phone", "mobile", "device"]),
    emoji!("💻", "laptop", Objects, ["computer", "laptop", "work"]),
    emoji!("⌚", "watch", Objects, ["watch", "time", "wearable"]),
    emoji!("📷", "camera", Objects, ["camera", "photo", "picture"]),
    emoji!("🎧", "headphone", Objects, ["headphones", "music", "audio"]),
    emoji!("🔋", "battery", Objects, ["battery", "power", "charge"]),
    emoji!(
        "🔌",
        "electric plug",
        Objects,
        ["plug", "power", "electric"]
    ),
    emoji!("💡", "light bulb", Objects, ["idea", "light", "bulb"]),
    emoji!(
        "🧯",
        "fire extinguisher",
        Objects,
        ["safety", "fire", "tool"]
    ),
    emoji!(
        "🛒",
        "shopping cart",
        Objects,
        ["shopping", "cart", "store"]
    ),
    emoji!(
        "🥹",
        "face holding back tears",
        SmileysEmotion,
        ["tears", "emotional", "moved"]
    ),
    emoji!(
        "🫠",
        "melting face",
        SmileysEmotion,
        ["melt", "awkward", "heat"]
    ),
    emoji!(
        "🫥",
        "dotted line face",
        SmileysEmotion,
        ["invisible", "faded", "awkward"]
    ),
    emoji!(
        "🫨",
        "shaking face",
        SmileysEmotion,
        ["shaking", "shocked", "vibrating"]
    ),
    emoji!(
        "🤥",
        "lying face",
        SmileysEmotion,
        ["lie", "pinocchio", "dishonest"]
    ),
    emoji!(
        "😮‍💨",
        "face exhaling",
        SmileysEmotion,
        ["exhale", "relief", "sigh"]
    ),
    emoji!(
        "😶‍🌫️",
        "face in clouds",
        SmileysEmotion,
        ["foggy", "confused", "dazed"]
    ),
    emoji!(
        "😵‍💫",
        "face with spiral eyes",
        SmileysEmotion,
        ["spiral", "dizzy", "hypnotized"]
    ),
    emoji!(
        "🫵",
        "index pointing at the viewer",
        PeopleBody,
        ["you", "point", "finger"]
    ),
    emoji!(
        "🫱",
        "rightwards hand",
        PeopleBody,
        ["hand", "right", "reach"]
    ),
    emoji!(
        "🫲",
        "leftwards hand",
        PeopleBody,
        ["hand", "left", "reach"]
    ),
    emoji!("🦶", "foot", PeopleBody, ["foot", "body", "kick"]),
    emoji!("🦵", "leg", PeopleBody, ["leg", "body", "step"]),
    emoji!(
        "🦻",
        "ear with hearing aid",
        PeopleBody,
        ["ear", "hearing", "accessibility"]
    ),
    emoji!("🫦", "biting lip", PeopleBody, ["lip", "nervous", "flirty"]),
    emoji!(
        "🫀",
        "anatomical heart",
        PeopleBody,
        ["heart", "organ", "anatomy"]
    ),
    emoji!("🐺", "wolf", AnimalsNature, ["wolf", "wild", "canine"]),
    emoji!("🐗", "boar", AnimalsNature, ["boar", "wild", "pig"]),
    emoji!(
        "🐴",
        "horse face",
        AnimalsNature,
        ["horse", "animal", "farm"]
    ),
    emoji!("🦄", "unicorn", AnimalsNature, ["unicorn", "magic", "myth"]),
    emoji!("🐔", "chicken", AnimalsNature, ["chicken", "bird", "farm"]),
    emoji!("🐧", "penguin", AnimalsNature, ["penguin", "bird", "cold"]),
    emoji!("🐦", "bird", AnimalsNature, ["bird", "animal", "tweet"]),
    emoji!("🐢", "turtle", AnimalsNature, ["turtle", "animal", "slow"]),
    emoji!(
        "🐬",
        "dolphin",
        AnimalsNature,
        ["dolphin", "ocean", "smart"]
    ),
    emoji!("🍌", "banana", FoodDrink, ["banana", "fruit", "food"]),
    emoji!("🍇", "grapes", FoodDrink, ["grapes", "fruit", "food"]),
    emoji!(
        "🍓",
        "strawberry",
        FoodDrink,
        ["strawberry", "fruit", "sweet"]
    ),
    emoji!("🥑", "avocado", FoodDrink, ["avocado", "fruit", "food"]),
    emoji!("🥓", "bacon", FoodDrink, ["bacon", "meat", "breakfast"]),
    emoji!("🍗", "poultry leg", FoodDrink, ["chicken", "meat", "food"]),
    emoji!("🍞", "bread", FoodDrink, ["bread", "food", "baked"]),
    emoji!("🧀", "cheese wedge", FoodDrink, ["cheese", "dairy", "food"]),
    emoji!("🍰", "shortcake", FoodDrink, ["cake", "dessert", "sweet"]),
    emoji!("🥗", "green salad", FoodDrink, ["salad", "healthy", "food"]),
    emoji!(
        "🚗",
        "automobile",
        TravelPlaces,
        ["car", "vehicle", "drive"]
    ),
    emoji!("🚕", "taxi", TravelPlaces, ["taxi", "cab", "vehicle"]),
    emoji!(
        "🚙",
        "sport utility vehicle",
        TravelPlaces,
        ["suv", "car", "vehicle"]
    ),
    emoji!("🚌", "bus", TravelPlaces, ["bus", "transit", "vehicle"]),
    emoji!(
        "🚎",
        "trolleybus",
        TravelPlaces,
        ["trolley", "bus", "transit"]
    ),
    emoji!(
        "🚓",
        "police car",
        TravelPlaces,
        ["police", "car", "emergency"]
    ),
    emoji!(
        "🚑",
        "ambulance",
        TravelPlaces,
        ["ambulance", "medical", "emergency"]
    ),
    emoji!(
        "🚒",
        "fire engine",
        TravelPlaces,
        ["fire", "truck", "emergency"]
    ),
    emoji!(
        "🚚",
        "delivery truck",
        TravelPlaces,
        ["truck", "delivery", "shipping"]
    ),
    emoji!("🚲", "bicycle", TravelPlaces, ["bike", "bicycle", "ride"]),
    emoji!(
        "✈️",
        "airplane",
        TravelPlaces,
        ["plane", "travel", "flight"]
    ),
    emoji!("🚀", "rocket", TravelPlaces, ["rocket", "space", "launch"]),
    emoji!(
        "🚂",
        "locomotive",
        TravelPlaces,
        ["train", "locomotive", "rail"]
    ),
    emoji!(
        "🚉",
        "railway station",
        TravelPlaces,
        ["station", "train", "travel"]
    ),
    emoji!("🏠", "house", TravelPlaces, ["house", "home", "building"]),
    emoji!("🏨", "hotel", TravelPlaces, ["hotel", "building", "travel"]),
    emoji!(
        "🗽",
        "statue of liberty",
        TravelPlaces,
        ["landmark", "nyc", "statue"]
    ),
    emoji!("⛵", "sailboat", TravelPlaces, ["boat", "sail", "water"]),
    emoji!(
        "⚽",
        "soccer ball",
        Activities,
        ["soccer", "football", "sport"]
    ),
    emoji!(
        "🏀",
        "basketball",
        Activities,
        ["basketball", "sport", "ball"]
    ),
    emoji!(
        "🏈",
        "american football",
        Activities,
        ["football", "sport", "nfl"]
    ),
    emoji!("⚾", "baseball", Activities, ["baseball", "sport", "ball"]),
    emoji!("🎾", "tennis", Activities, ["tennis", "sport", "racket"]),
    emoji!(
        "🏐",
        "volleyball",
        Activities,
        ["volleyball", "sport", "ball"]
    ),
    emoji!(
        "🏓",
        "ping pong",
        Activities,
        ["pingpong", "table tennis", "sport"]
    ),
    emoji!(
        "🏸",
        "badminton",
        Activities,
        ["badminton", "sport", "racket"]
    ),
    emoji!(
        "🥊",
        "boxing glove",
        Activities,
        ["boxing", "fight", "sport"]
    ),
    emoji!(
        "🎮",
        "video game",
        Activities,
        ["gaming", "controller", "play"]
    ),
    emoji!("🎯", "direct hit", Activities, ["target", "dart", "game"]),
    emoji!("🎲", "game die", Activities, ["dice", "game", "luck"]),
    emoji!(
        "🖥️",
        "desktop computer",
        Objects,
        ["desktop", "computer", "monitor"]
    ),
    emoji!("🖨️", "printer", Objects, ["printer", "print", "office"]),
    emoji!(
        "🕹️",
        "joystick",
        Objects,
        ["joystick", "game", "controller"]
    ),
    emoji!("💽", "computer disk", Objects, ["disk", "storage", "data"]),
    emoji!("📺", "television", Objects, ["tv", "screen", "video"]),
    emoji!("📚", "books", Objects, ["books", "study", "read"]),
    emoji!("✏️", "pencil", Objects, ["pencil", "write", "school"]),
    emoji!("🧰", "toolbox", Objects, ["toolbox", "tools", "repair"]),
    emoji!("🧲", "magnet", Objects, ["magnet", "science", "metal"]),
    emoji!("🧪", "test tube", Objects, ["test", "science", "lab"]),
    emoji!("✨", "sparkles", Symbols, ["sparkle", "shine", "magic"]),
    emoji!("⭐", "star", Symbols, ["star", "favorite", "rating"]),
    emoji!("🌟", "glowing star", Symbols, ["star", "glow", "sparkle"]),
    emoji!("🔔", "bell", Symbols, ["bell", "notification", "alert"]),
    emoji!("🎵", "musical note", Symbols, ["music", "note", "song"]),
    emoji!("✅", "check mark button", Symbols, ["check", "done", "yes"]),
    emoji!("❌", "cross mark", Symbols, ["cross", "no", "cancel"]),
    emoji!("⚠️", "warning", Symbols, ["warning", "alert", "caution"]),
    emoji!(
        "🚫",
        "prohibited",
        Symbols,
        ["prohibited", "no", "forbidden"]
    ),
    emoji!(
        "♻️",
        "recycling symbol",
        Symbols,
        ["recycle", "green", "eco"]
    ),
    emoji!("🆗", "OK button", Symbols, ["ok", "button", "agree"]),
    emoji!(
        "🇺🇸",
        "flag: United States",
        Flags,
        ["flag", "usa", "america"]
    ),
    emoji!("🇨🇦", "flag: Canada", Flags, ["flag", "canada", "country"]),
    emoji!(
        "🇬🇧",
        "flag: United Kingdom",
        Flags,
        ["flag", "uk", "britain"]
    ),
    emoji!("🇫🇷", "flag: France", Flags, ["flag", "france", "country"]),
    emoji!("🇩🇪", "flag: Germany", Flags, ["flag", "germany", "country"]),
    emoji!("🇯🇵", "flag: Japan", Flags, ["flag", "japan", "country"]),
    emoji!(
        "🇰🇷",
        "flag: South Korea",
        Flags,
        ["flag", "korea", "country"]
    ),
    emoji!("🇮🇳", "flag: India", Flags, ["flag", "india", "country"]),
    emoji!("🇧🇷", "flag: Brazil", Flags, ["flag", "brazil", "country"]),
    emoji!(
        "🇦🇺",
        "flag: Australia",
        Flags,
        ["flag", "australia", "country"]
    ),
];

pub fn emojis_by_category(category: EmojiCategory) -> Vec<&'static Emoji> {
    EMOJIS
        .iter()
        .filter(|emoji| emoji.category == category)
        .collect()
}

pub fn grouped_emojis() -> Vec<(EmojiCategory, Vec<&'static Emoji>)> {
    all_categories()
        .map(|category| (category, emojis_by_category(category)))
        .collect()
}

/// Number of columns in the emoji picker grid.
/// Shared between the renderer and the arrow-key navigation interceptor.
pub const GRID_COLS: usize = 12;

/// Fixed square size for each rendered emoji tile.
pub const GRID_TILE_SIZE: f32 = 48.0;

/// Horizontal spacing between emoji tiles. Combined with the tile size, this
/// matches the row height so horizontal and vertical centers share one rhythm.
pub const GRID_TILE_GAP: f32 = 8.0;

/// Fixed height for every emoji picker row (headers and cell rows).
pub const GRID_ROW_HEIGHT: f32 = 56.0;

/// Glyph scale relative to the tile, keeping emoji sizing derived from the
/// shared tile size instead of introducing a renderer-local pixel value.
pub const GRID_GLYPH_SCALE: f32 = 0.75;

/// Label rendered as the section header above the Frequently Used head
/// block. Pulled into a constant so the renderer, audits, and tests all
/// agree on the exact text.
pub const FREQUENTLY_USED_LABEL: &str = "Frequently Used";

/// Build the filtered, category-ordered emoji list used by both the renderer
/// and the arrow-key navigation handler. This ensures selection indices stay
/// in sync between rendering and navigation.
pub fn filtered_ordered_emojis(
    filter: &str,
    selected_category: Option<EmojiCategory>,
) -> Vec<Emoji> {
    let mut filtered: Vec<Emoji> = search_emojis(filter).into_iter().copied().collect();
    if let Some(cat) = selected_category {
        filtered.retain(|e| e.category == cat);
    }
    let mut ordered: Vec<Emoji> = Vec::with_capacity(filtered.len());
    for cat in all_categories() {
        ordered.extend(filtered.iter().copied().filter(|e| e.category == cat));
    }
    ordered
}

/// Return the canonical EMOJIS slice index for an emoji string, used as the
/// final stable tie-break when ranking frequent emojis. `None` for unknown
/// strings so callers can sort them last.
pub fn dataset_order_of(emoji: &str) -> Option<usize> {
    EMOJIS.iter().position(|e| e.emoji == emoji)
}

/// Look up the canonical Emoji record for a given string, or `None` if not in
/// the dataset (e.g. a usage entry left over from a prior dataset).
pub fn emoji_by_value(emoji: &str) -> Option<Emoji> {
    EMOJIS.iter().find(|e| e.emoji == emoji).copied()
}

/// Composite display order used by render + navigation + Enter commit. When
/// `filter` is empty, `selected_category` is `None`, and `frequent` is
/// non-empty, the returned vector has two logical sections:
///
/// 1. **Frequently Used** — resolved from `frequent` in order; silently drops
///    unknown strings. (`frequent_count` on the returned `DisplayOrder`.)
/// 2. **Categories** — the normal category-ordered list with any emoji that
///    appeared in section 1 removed so the user never sees the same glyph
///    twice.
///
/// Every other combination (non-empty filter, explicit category, empty
/// snapshot) falls back to the existing category-ordered list with
/// `frequent_count == 0`, so search and category browsing stay predictable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayOrder {
    pub emojis: Vec<Emoji>,
    /// Number of leading emojis that come from the Frequently Used section.
    pub frequent_count: usize,
}

pub fn display_ordered_emojis(
    filter: &str,
    selected_category: Option<EmojiCategory>,
    frequent: &[String],
) -> DisplayOrder {
    let base = filtered_ordered_emojis(filter, selected_category);

    let can_show_frequent =
        filter.trim().is_empty() && selected_category.is_none() && !frequent.is_empty();

    if !can_show_frequent {
        return DisplayOrder {
            emojis: base,
            frequent_count: 0,
        };
    }

    let mut seen = std::collections::HashSet::new();
    let mut frequent_emojis: Vec<Emoji> = Vec::with_capacity(frequent.len());
    for value in frequent {
        if !seen.insert(value.clone()) {
            continue;
        }
        if let Some(emoji) = emoji_by_value(value) {
            frequent_emojis.push(emoji);
        }
    }

    let mut combined = Vec::with_capacity(frequent_emojis.len() + base.len());
    let frequent_count = frequent_emojis.len();
    combined.extend_from_slice(&frequent_emojis);
    combined.extend(base.into_iter().filter(|e| !seen.contains(e.emoji)));
    DisplayOrder {
        emojis: combined,
        frequent_count,
    }
}

/// Compute the uniform-list row index that contains `selected_index`.
///
/// Row layout per category: 1 header row + ceil(count / GRID_COLS) cell rows.
/// Returns the row suitable for `scroll_handle.scroll_to_item(row, Nearest)`.
pub fn compute_scroll_row(selected_index: usize, ordered: &[Emoji]) -> usize {
    let cols = GRID_COLS;
    let mut flat_offset: usize = 0;
    let mut row_offset: usize = 0;
    for cat in all_categories() {
        let cat_count = ordered.iter().filter(|e| e.category == cat).count();
        if cat_count == 0 {
            continue;
        }
        if flat_offset + cat_count > selected_index {
            let idx_in_cat = selected_index - flat_offset;
            let cell_row = idx_in_cat / cols;
            row_offset += 1 + cell_row;
            return row_offset;
        }
        row_offset += 1 + cat_count.div_ceil(cols);
        flat_offset += cat_count;
    }
    row_offset
}

/// Return the number of rendered grid rows (headers + cell rows) for a given
/// filter and optional category. Used by window sizing so the picker is sized
/// by actual visual rows, not raw emoji count.
pub fn filtered_grid_row_count(filter: &str, selected_category: Option<EmojiCategory>) -> usize {
    let ordered = filtered_ordered_emojis(filter, selected_category);
    let mut row_count = 0;
    let mut flat_offset = 0;

    for category in all_categories() {
        let category_count = ordered[flat_offset..]
            .iter()
            .take_while(|e| e.category == category)
            .count();
        if category_count == 0 {
            continue;
        }
        // 1 header row + ceil(count / GRID_COLS) cell rows
        row_count += 1 + category_count.div_ceil(GRID_COLS);
        flat_offset += category_count;
    }

    row_count
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmojiNavDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmojiCellRow {
    pub visible_row_index: usize,
    pub start_index: usize,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmojiGridLayout {
    pub rows: Vec<EmojiCellRow>,
    pub item_to_row: Vec<usize>,
}

/// Build a grid layout for a display-ordered emoji list that may have a
/// leading "Frequently Used" head block.
///
/// The head block is rendered under a single synthetic section header and
/// treated as one contiguous region for Up/Down navigation. The remaining
/// emojis are grouped by category exactly like [`build_emoji_grid_layout`].
///
/// Passing `frequent_count == 0` produces the same layout as calling
/// [`build_emoji_grid_layout`] directly, so callers can unconditionally
/// route through this helper without branching on snapshot presence.
pub fn build_display_grid_layout(display: &DisplayOrder, cols: usize) -> EmojiGridLayout {
    let emojis = &display.emojis;
    let frequent_count = display.frequent_count.min(emojis.len());

    let mut rows: Vec<EmojiCellRow> = Vec::new();
    let mut item_to_row = vec![0usize; emojis.len()];
    let mut visible_row_index = 0usize;

    if frequent_count > 0 {
        visible_row_index += 1;
        let mut row_offset = 0usize;
        while row_offset < frequent_count {
            let count = (frequent_count - row_offset).min(cols);
            let start_index = row_offset;
            let cell_row_index = rows.len();

            rows.push(EmojiCellRow {
                visible_row_index,
                start_index,
                count,
            });

            for entry in item_to_row.iter_mut().skip(start_index).take(count) {
                *entry = cell_row_index;
            }

            visible_row_index += 1;
            row_offset += count;
        }
    }

    let mut flat_offset = frequent_count;
    for category in all_categories() {
        let category_count = emojis[flat_offset..]
            .iter()
            .take_while(|emoji| emoji.category == category)
            .count();

        if category_count == 0 {
            continue;
        }

        visible_row_index += 1;

        let mut row_offset = 0usize;
        while row_offset < category_count {
            let count = (category_count - row_offset).min(cols);
            let start_index = flat_offset + row_offset;
            let cell_row_index = rows.len();

            rows.push(EmojiCellRow {
                visible_row_index,
                start_index,
                count,
            });

            for entry in item_to_row.iter_mut().skip(start_index).take(count) {
                *entry = cell_row_index;
            }

            visible_row_index += 1;
            row_offset += count;
        }

        flat_offset += category_count;
    }

    EmojiGridLayout { rows, item_to_row }
}

/// Single-step scroll row helper mirroring [`compute_scroll_row`] for a
/// [`DisplayOrder`]. Returns the visible row index of `selected_index`
/// accounting for the Frequently Used head block (if any) and category
/// header rows.
pub fn compute_display_scroll_row(selected_index: usize, display: &DisplayOrder) -> usize {
    let cols = GRID_COLS;
    let emojis = &display.emojis;
    let frequent_count = display.frequent_count.min(emojis.len());

    if selected_index < frequent_count {
        // 1 header row for "Frequently Used" + cell-row offset within the block.
        return 1 + selected_index / cols;
    }

    let head_rows = if frequent_count > 0 {
        1 + frequent_count.div_ceil(cols)
    } else {
        0
    };

    let tail_index = selected_index - frequent_count;
    let mut flat_offset: usize = 0;
    let mut row_offset: usize = head_rows;
    for cat in all_categories() {
        let cat_count = emojis[frequent_count..]
            .iter()
            .filter(|e| e.category == cat)
            .count();
        if cat_count == 0 {
            continue;
        }
        if flat_offset + cat_count > tail_index {
            let idx_in_cat = tail_index - flat_offset;
            let cell_row = idx_in_cat / cols;
            row_offset += 1 + cell_row;
            return row_offset;
        }
        row_offset += 1 + cat_count.div_ceil(cols);
        flat_offset += cat_count;
    }
    row_offset
}

pub fn build_emoji_grid_layout<T>(
    ordered_emojis: &[T],
    cols: usize,
    category_of: impl Fn(&T) -> EmojiCategory,
) -> EmojiGridLayout {
    let mut rows = Vec::new();
    let mut item_to_row = vec![0; ordered_emojis.len()];
    let mut flat_offset = 0usize;
    let mut visible_row_index = 0usize;

    for category in all_categories() {
        let category_count = ordered_emojis[flat_offset..]
            .iter()
            .take_while(|emoji| category_of(emoji) == category)
            .count();

        if category_count == 0 {
            continue;
        }

        // Skip the header row for this category.
        visible_row_index += 1;

        let mut row_offset = 0usize;
        while row_offset < category_count {
            let count = (category_count - row_offset).min(cols);
            let start_index = flat_offset + row_offset;
            let cell_row_index = rows.len();

            rows.push(EmojiCellRow {
                visible_row_index,
                start_index,
                count,
            });

            for entry in item_to_row.iter_mut().skip(start_index).take(count) {
                *entry = cell_row_index;
            }

            visible_row_index += 1;
            row_offset += count;
        }

        flat_offset += category_count;
    }

    EmojiGridLayout { rows, item_to_row }
}

impl EmojiGridLayout {
    pub fn scroll_row_for_index(&self, index: usize) -> usize {
        self.item_to_row
            .get(index)
            .and_then(|row_ix| self.rows.get(*row_ix))
            .map(|row| row.visible_row_index)
            .unwrap_or(0)
    }

    pub fn move_index(&self, index: usize, direction: EmojiNavDirection) -> usize {
        let Some(&cell_row_index) = self.item_to_row.get(index) else {
            return 0;
        };
        let Some(row) = self.rows.get(cell_row_index) else {
            return index;
        };

        let column = index.saturating_sub(row.start_index);

        match direction {
            EmojiNavDirection::Left => {
                if column > 0 {
                    index - 1
                } else if cell_row_index > 0 {
                    let prev = &self.rows[cell_row_index - 1];
                    prev.start_index + prev.count.saturating_sub(1)
                } else {
                    index
                }
            }
            EmojiNavDirection::Right => {
                if column + 1 < row.count {
                    index + 1
                } else if let Some(next) = self.rows.get(cell_row_index + 1) {
                    next.start_index
                } else {
                    index
                }
            }
            EmojiNavDirection::Up => {
                if cell_row_index == 0 {
                    index
                } else {
                    let prev = &self.rows[cell_row_index - 1];
                    prev.start_index + column.min(prev.count.saturating_sub(1))
                }
            }
            EmojiNavDirection::Down => {
                if let Some(next) = self.rows.get(cell_row_index + 1) {
                    next.start_index + column.min(next.count.saturating_sub(1))
                } else {
                    index
                }
            }
        }
    }
}

pub fn search_emojis(query: &str) -> Vec<&Emoji> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return EMOJIS.iter().collect();
    }

    EMOJIS
        .iter()
        .filter(|emoji| {
            emoji.name.to_ascii_lowercase().contains(&query)
                || emoji
                    .keywords
                    .iter()
                    .any(|keyword| keyword.to_ascii_lowercase().contains(&query))
        })
        .collect()
}

#[cfg(test)]
include!("mod_tests.rs");
