//! Curated knowledge used to classify apps: friendly names, categories, removal recommendations
//! and consequences. Anything not listed falls back to conservative heuristics.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Microsoft,
    Gaming,
    ThirdParty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Recommendation {
    /// Preinstalled extra that Windows works fine without; reinstallable.
    Safe,
    /// An app you installed or may use; remove if you don't need it.
    Optional,
    /// Removing affects other apps, hardware, games or Windows features. Read the note.
    Caution,
    /// Part of Windows or required by other software. Never offered for removal.
    Protected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppKind {
    App,
    Runtime,
    Driver,
    System,
}

pub struct AppxRule {
    /// Package name; a trailing `*` matches as prefix. Case-insensitive.
    pub pattern: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub rec: Recommendation,
    pub kind: AppKind,
    pub note: &'static str,
    /// Microsoft Store product ID for reinstalling through winget.
    pub store_id: Option<&'static str>,
}

use AppKind as K;
use Category as C;
use Recommendation as R;

const fn rule(
    pattern: &'static str,
    name: &'static str,
    category: Category,
    rec: Recommendation,
    kind: AppKind,
    note: &'static str,
    store_id: Option<&'static str>,
) -> AppxRule {
    AppxRule { pattern, name, category, rec, kind, note, store_id }
}

const CODEC: &str = "Media codec. Removing it stops Photos, Media Player and other apps from opening files in this format.";

pub static APPX_RULES: &[AppxRule] = &[
    // ---- Never removable: Windows depends on these ----
    rule("Microsoft.WindowsStore", "Microsoft Store", C::Microsoft, R::Protected, K::System, "Needed to install and update Store apps, including the ones Blob Optimizers can reinstall.", None),
    rule("Microsoft.StorePurchaseApp", "Store Purchase", C::Microsoft, R::Protected, K::System, "Required by the Microsoft Store.", None),
    rule("Microsoft.DesktopAppInstaller", "App Installer (winget)", C::Microsoft, R::Protected, K::System, "Provides winget, used to install, update and reinstall apps.", None),
    rule("Microsoft.Winget.Source", "winget source", C::Microsoft, R::Protected, K::System, "Package catalogue used by winget.", None),
    rule("Microsoft.SecHealthUI", "Windows Security", C::Microsoft, R::Protected, K::System, "The Windows Security app (Defender, firewall). Never removed.", None),
    rule("Microsoft.MicrosoftEdge*", "Microsoft Edge", C::Microsoft, R::Protected, K::System, "Windows and many apps use Edge components; removing it is unsupported.", None),
    rule("Microsoft.WindowsAppRuntime*", "Windows App Runtime", C::Microsoft, R::Protected, K::Runtime, "Shared runtime required by many apps.", None),
    rule("MicrosoftCorporationII.WinAppRuntime*", "Windows App Runtime", C::Microsoft, R::Protected, K::Runtime, "Shared runtime required by many apps.", None),
    rule("Microsoft.VCLibs*", "Visual C++ runtime", C::Microsoft, R::Protected, K::Runtime, "Shared runtime required by Store apps.", None),
    rule("Microsoft.UI.Xaml*", "UI framework", C::Microsoft, R::Protected, K::Runtime, "Shared UI framework required by Store apps.", None),
    rule("Microsoft.NET.*", ".NET runtime", C::Microsoft, R::Protected, K::Runtime, "Shared runtime required by Store apps.", None),
    rule("Microsoft.Services.Store.Engagement", "Store engagement", C::Microsoft, R::Protected, K::Runtime, "Shared Store component.", None),
    rule("Microsoft.LanguageExperiencePack*", "Language pack", C::Microsoft, R::Protected, K::System, "Display language for Windows. Remove languages from Windows language settings instead.", None),
    rule("Microsoft.ApplicationCompatibilityEnhancements", "App compatibility enhancements", C::Microsoft, R::Protected, K::System, "Compatibility fixes used by Windows.", None),
    rule("MicrosoftWindows.Client.WebExperience", "Widgets", C::Microsoft, R::Caution, K::App, "Powers the Widgets board and taskbar weather. Removing it removes Widgets.", Some("9MSSGKG348SP")),
    rule("MicrosoftWindows.Client.*", "Windows shell component", C::Microsoft, R::Protected, K::System, "Core part of the Windows shell (Start, taskbar, File Explorer).", None),

    // ---- Preinstalled extras that are safe to remove (reinstallable from the Store) ----
    rule("Microsoft.BingNews", "Microsoft News", C::Microsoft, R::Safe, K::App, "News reader. Nothing else depends on it.", Some("9WZDNCRFHVFW")),
    rule("Microsoft.BingWeather", "Weather", C::Microsoft, R::Safe, K::App, "Weather app. The taskbar weather widget is separate (Widgets).", Some("9WZDNCRFJ3Q2")),
    rule("Microsoft.BingFinance", "Money", C::Microsoft, R::Safe, K::App, "Discontinued finance app.", None),
    rule("Microsoft.BingSports", "Sports", C::Microsoft, R::Safe, K::App, "Discontinued sports app.", None),
    rule("Microsoft.BingSearch", "Bing Search", C::Microsoft, R::Safe, K::App, "Bing web search integration app. Windows Search keeps working.", None),
    rule("Microsoft.GetHelp", "Get Help", C::Microsoft, R::Safe, K::App, "Microsoft support app. Troubleshooters inside Windows are not affected.", Some("9PKDZBMV1H3T")),
    rule("Microsoft.Getstarted", "Tips", C::Microsoft, R::Safe, K::App, "Windows tips and onboarding app.", None),
    rule("Microsoft.MicrosoftSolitaireCollection", "Solitaire Collection", C::Gaming, R::Safe, K::App, "Ad-supported card games.", Some("9WZDNCRFHWD2")),
    rule("Microsoft.MicrosoftOfficeHub", "Microsoft 365 (Office hub)", C::Microsoft, R::Safe, K::App, "Launcher for Office web apps. Installed Office apps are not affected.", Some("9WZDNCRD29V9")),
    rule("Microsoft.People", "People", C::Microsoft, R::Safe, K::App, "Legacy contacts app.", None),
    rule("Microsoft.WindowsFeedbackHub", "Feedback Hub", C::Microsoft, R::Safe, K::App, "Sends feedback to Microsoft.", Some("9NBLGGH4R32N")),
    rule("Microsoft.WindowsMaps", "Maps", C::Microsoft, R::Safe, K::App, "Offline maps app (discontinued).", Some("9WZDNCRDTBVB")),
    rule("Microsoft.ZuneVideo", "Movies & TV", C::Microsoft, R::Safe, K::App, "Video player/store. Media Player can play videos instead.", Some("9WZDNCRFJ3P2")),
    rule("Microsoft.Messaging", "Messaging", C::Microsoft, R::Safe, K::App, "Legacy messaging app.", None),
    rule("Microsoft.Print3D", "Print 3D", C::Microsoft, R::Safe, K::App, "Discontinued 3D printing app.", None),
    rule("Microsoft.3DBuilder", "3D Builder", C::Microsoft, R::Safe, K::App, "Discontinued 3D modelling app.", None),
    rule("Microsoft.Microsoft3DViewer", "3D Viewer", C::Microsoft, R::Safe, K::App, "3D model viewer.", None),
    rule("Microsoft.MixedReality.Portal", "Mixed Reality Portal", C::Microsoft, R::Safe, K::App, "Only needed for Windows Mixed Reality headsets.", None),
    rule("Microsoft.Wallet", "Wallet", C::Microsoft, R::Safe, K::App, "Legacy payments app.", None),
    rule("Microsoft.OneConnect", "Mobile Plans", C::Microsoft, R::Safe, K::App, "Cellular data plans app.", None),
    rule("Microsoft.SkypeApp", "Skype", C::Microsoft, R::Safe, K::App, "Skype has been discontinued.", None),
    rule("Microsoft.549981C3F5F10", "Cortana", C::Microsoft, R::Safe, K::App, "Discontinued assistant.", None),
    rule("Microsoft.Todos", "Microsoft To Do", C::Microsoft, R::Safe, K::App, "Task list app. Your tasks stay in your Microsoft account.", Some("9NBLGGH5R558")),
    rule("Microsoft.PowerAutomateDesktop", "Power Automate", C::Microsoft, R::Safe, K::App, "Automation tool; remove if you don't build flows.", Some("9NFTCH6J7FHV")),
    rule("Clipchamp.Clipchamp", "Clipchamp", C::Microsoft, R::Safe, K::App, "Video editor.", Some("9P1J8S7CCWWT")),
    rule("Microsoft.OutlookForWindows", "Outlook (new)", C::Microsoft, R::Optional, K::App, "Mail and calendar. Remove only if you use another mail app.", Some("9NRX63209R7B")),
    rule("microsoft.windowscommunicationsapps", "Mail and Calendar", C::Microsoft, R::Safe, K::App, "Retired mail app replaced by the new Outlook.", None),
    rule("MSTeams", "Microsoft Teams", C::Microsoft, R::Optional, K::App, "Remove if you don't use Teams for chat or meetings.", Some("XP8BT8DW290MPQ")),
    rule("MicrosoftTeams", "Teams (personal)", C::Microsoft, R::Safe, K::App, "Consumer Teams chat built into Windows 11.", None),
    rule("Microsoft.Windows.DevHome", "Dev Home", C::Microsoft, R::Safe, K::App, "Developer dashboard (retired by Microsoft).", None),
    rule("Microsoft.Copilot", "Copilot", C::Microsoft, R::Optional, K::App, "AI assistant app. Remove if you don't use it.", Some("9NHT9RB2F4HD")),
    rule("Microsoft.MicrosoftJournal", "Journal", C::Microsoft, R::Safe, K::App, "Pen note-taking app.", None),
    rule("Microsoft.StartExperiencesApp", "Start menu companion", C::Microsoft, R::Caution, K::App, "Provides Start menu extras such as the phone companion panel. Start keeps working without it.", None),

    // ---- Built-in tools people often use ----
    rule("Microsoft.WindowsCalculator", "Calculator", C::Microsoft, R::Caution, K::App, "Windows' calculator. Removing leaves no calculator unless you install another.", Some("9WZDNCRFHVN5")),
    rule("Microsoft.WindowsCamera", "Camera", C::Microsoft, R::Caution, K::App, "Camera app. Other apps can still use the webcam.", Some("9WZDNCRFJBBG")),
    rule("Microsoft.WindowsAlarms", "Clock", C::Microsoft, R::Caution, K::App, "Alarms, timers and focus sessions.", Some("9WZDNCRFJ3PR")),
    rule("Microsoft.WindowsSoundRecorder", "Sound Recorder", C::Microsoft, R::Caution, K::App, "Voice recording app.", Some("9WZDNCRFHWKN")),
    rule("Microsoft.Paint", "Paint", C::Microsoft, R::Caution, K::App, "Image editor.", Some("9PCFS5B6T72H")),
    rule("Microsoft.WindowsNotepad", "Notepad", C::Microsoft, R::Caution, K::App, "Text editor; other apps open .txt files with it.", Some("9MSMLRH6LZF3")),
    rule("Microsoft.ScreenSketch", "Snipping Tool", C::Microsoft, R::Caution, K::App, "Screenshots and screen recording (Win+Shift+S).", Some("9MZ95KL8MR0L")),
    rule("Microsoft.Windows.Photos", "Photos", C::Microsoft, R::Caution, K::App, "Default image viewer. Install another viewer first.", Some("9WZDNCRFJBH4")),
    rule("Microsoft.ZuneMusic", "Media Player", C::Microsoft, R::Caution, K::App, "Default music and video player.", Some("9WZDNCRFJ3PT")),
    rule("Microsoft.WindowsTerminal", "Terminal", C::Microsoft, R::Caution, K::App, "Windows Terminal. Command prompts fall back to the classic console.", Some("9N0DX20HK701")),
    rule("Microsoft.MicrosoftStickyNotes", "Sticky Notes", C::Microsoft, R::Caution, K::App, "Notes sync with your Microsoft account if you signed in.", Some("9NBLGGH4QGHW")),
    rule("Microsoft.YourPhone", "Phone Link", C::Microsoft, R::Optional, K::App, "Connects an Android phone or iPhone to the PC.", Some("9NMPJ99VJBWV")),
    rule("MicrosoftWindows.CrossDevice", "Cross Device Experience Host", C::Microsoft, R::Caution, K::App, "Used by Phone Link and mobile device features.", None),
    rule("MicrosoftCorporationII.QuickAssist", "Quick Assist", C::Microsoft, R::Optional, K::App, "Remote help tool; remove if nobody helps you remotely.", Some("9P7BP5VNWKX5")),

    // ---- Codecs ----
    rule("Microsoft.HEIFImageExtension", "HEIF Image Extensions", C::Microsoft, R::Caution, K::Runtime, CODEC, Some("9PMMSR1CGPWG")),
    rule("Microsoft.HEVCVideoExtension", "HEVC Video Extensions", C::Microsoft, R::Caution, K::Runtime, CODEC, None),
    rule("Microsoft.AV1VideoExtension", "AV1 Video Extension", C::Microsoft, R::Caution, K::Runtime, CODEC, Some("9MVZQVXJBQ9V")),
    rule("Microsoft.AVCEncoderVideoExtension", "AVC Encoder Extension", C::Microsoft, R::Caution, K::Runtime, CODEC, None),
    rule("Microsoft.VP9VideoExtensions", "VP9 Video Extensions", C::Microsoft, R::Caution, K::Runtime, CODEC, Some("9N4D0MSMP0PT")),
    rule("Microsoft.WebMediaExtensions", "Web Media Extensions", C::Microsoft, R::Caution, K::Runtime, CODEC, Some("9N5TDP8VCMHS")),
    rule("Microsoft.WebpImageExtension", "WebP Image Extensions", C::Microsoft, R::Caution, K::Runtime, CODEC, Some("9PG2DK419DRG")),
    rule("Microsoft.RawImageExtension", "Raw Image Extension", C::Microsoft, R::Caution, K::Runtime, CODEC, Some("9NCTDW2W1BH8")),
    rule("Microsoft.MPEG2VideoExtension", "MPEG-2 Video Extension", C::Microsoft, R::Caution, K::Runtime, CODEC, None),

    // ---- Gaming ----
    rule("Microsoft.GamingApp", "Xbox app", C::Gaming, R::Caution, K::App, "Needed to install and launch PC Game Pass games.", Some("9MV0B5HZVK9Z")),
    rule("Microsoft.GamingServices", "Gaming Services", C::Gaming, R::Caution, K::Runtime, "Required by Game Pass and Microsoft Store games; those games will not start without it.", None),
    rule("Microsoft.XboxGamingOverlay", "Xbox Game Bar", C::Gaming, R::Caution, K::App, "Win+G overlay: captures, performance widgets, party chat. Some games use it for screenshots.", Some("9NZKPSTSNW4P")),
    rule("Microsoft.XboxIdentityProvider", "Xbox Identity Provider", C::Gaming, R::Caution, K::Runtime, "Signs PC games into the Xbox network (e.g. Minecraft, Forza, Sea of Thieves).", Some("9WZDNCRD1HKW")),
    rule("Microsoft.XboxSpeechToTextOverlay", "Xbox speech-to-text", C::Gaming, R::Safe, K::App, "Accessibility transcription for Xbox party chat.", None),
    rule("Microsoft.Xbox.TCUI", "Xbox TCUI", C::Gaming, R::Caution, K::Runtime, "Xbox profile/friends dialogs used inside some games.", None),
    rule("Microsoft.XboxApp", "Xbox Console Companion", C::Gaming, R::Safe, K::App, "Retired console companion app (replaced by the Xbox app).", None),
    rule("Microsoft.XboxGameOverlay", "Xbox Game overlay (legacy)", C::Gaming, R::Safe, K::App, "Legacy overlay component.", None),
    rule("Microsoft.MinecraftUWP", "Minecraft for Windows", C::Gaming, R::Optional, K::App, "A game you installed. Worlds are kept in your profile.", None),
    rule("Microsoft.4297127D64EC6", "Minecraft Launcher", C::Gaming, R::Optional, K::App, "Launcher for Minecraft Java and Bedrock editions.", None),

    // ---- Driver companion apps ----
    rule("AdvancedMicroDevicesInc-RSXCM", "AMD Software component", C::ThirdParty, R::Caution, K::Driver, "Part of AMD Software (Adrenalin). Removing it can break GPU settings and features.", None),
    rule("AdvancedMicroDevicesInc*", "AMD component", C::ThirdParty, R::Caution, K::Driver, "AMD driver companion app.", None),
    rule("NVIDIACorp.NVIDIAControlPanel", "NVIDIA Control Panel", C::ThirdParty, R::Caution, K::Driver, "GPU settings (resolution, G-SYNC, 3D settings).", None),
    rule("RealtekSemiconductorCorp.*", "Realtek Audio Console", C::ThirdParty, R::Caution, K::Driver, "Audio driver settings app.", None),
    rule("AppUp.IntelGraphicsExperience", "Intel Graphics Command Center", C::ThirdParty, R::Caution, K::Driver, "Intel GPU settings.", None),
    rule("DolbyLaboratories.*", "Dolby audio", C::ThirdParty, R::Caution, K::Driver, "Audio enhancement app tied to the sound driver.", None),
    rule("WavesAudio.*", "Waves audio", C::ThirdParty, R::Caution, K::Driver, "Audio enhancement app tied to the sound driver.", None),

    // ---- Third-party apps Windows installs automatically ----
    rule("king.com.*", "King games (Candy Crush)", C::Gaming, R::Safe, K::App, "Promotional game installed by Windows.", None),
    rule("SpotifyAB.SpotifyMusic", "Spotify", C::ThirdParty, R::Optional, K::App, "Often preinstalled. Remove if you don't use it.", None),
    rule("Disney.*", "Disney+", C::ThirdParty, R::Safe, K::App, "Promotional streaming app.", None),
    rule("Facebook.*", "Facebook", C::ThirdParty, R::Safe, K::App, "Promotional app.", None),
    rule("BytedancePte.Ltd.TikTok", "TikTok", C::ThirdParty, R::Safe, K::App, "Promotional app.", None),
    rule("AmazonVideo.PrimeVideo", "Prime Video", C::ThirdParty, R::Safe, K::App, "Promotional streaming app.", None),
    rule("4DF9E0F8.Netflix", "Netflix", C::ThirdParty, R::Safe, K::App, "Promotional streaming app.", None),
    rule("Microsoft.Office.OneNote", "OneNote for Windows 10", C::Microsoft, R::Safe, K::App, "Retired OneNote app; notes live in OneDrive.", None),

    // ---- Catch-alls last: specific rules above take precedence ----
    rule("Microsoft.Windows.*", "Windows component", C::Microsoft, R::Protected, K::System, "Built-in Windows component.", None),
];

/// Packages that have a rule but are installed by the user, not shipped with Windows.
const USER_INSTALLED: &[&str] = &["Microsoft.MinecraftUWP", "Microsoft.4297127D64EC6"];

pub fn is_user_installed(name: &str) -> bool {
    USER_INSTALLED.iter().any(|n| n.eq_ignore_ascii_case(name))
}

pub fn appx_rule(name: &str) -> Option<&'static AppxRule> {
    let lower = name.to_ascii_lowercase();
    APPX_RULES.iter().find(|r| {
        let p = r.pattern.to_ascii_lowercase();
        match p.strip_suffix('*') {
            Some(prefix) => lower.starts_with(prefix),
            None => lower == p,
        }
    })
}

/// Turns "Microsoft.WindowsSoundRecorder" into "Windows Sound Recorder" when nothing better is known.
pub fn derive_name(package: &str) -> String {
    let last = package.rsplit('.').next().unwrap_or(package);
    let mut out = String::new();
    let mut prev_lower = false;
    for c in last.chars() {
        if c.is_ascii_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        out.push(c);
    }
    out
}

pub struct Win32Class {
    pub category: Category,
    pub rec: Recommendation,
    pub kind: AppKind,
    pub note: &'static str,
    pub preinstalled: bool,
}

fn re(p: &str) -> Regex {
    Regex::new(&format!("(?i){p}")).expect("valid knowledge regex")
}

static PROTECTED: LazyLock<Regex> = LazyLock::new(|| {
    re(r"^(?:Blob Optimizers|Microsoft Edge(?: WebView2 Runtime| Update)?|Microsoft Update Health Tools|Windows Defender|Microsoft Defender)\b")
});
static RUNTIME: LazyLock<Regex> = LazyLock::new(|| {
    re(r"Visual C\+\+|\.NET|Desktop Runtime|Windows App Runtime|WinAppRuntime|DirectX|GameInput|Xbox Service|\bJava\b|\bJDK\b|\bJRE\b|Python \d|Node\.js|^vs_|Software Development Kit|Windows SDK|Driver Kit|Application Compatibility|Visual Studio Installer|Redistributable|Runtime\b")
});
static DRIVER: LazyLock<Regex> = LazyLock::new(|| {
    re(r"\bdrivers?\b|chipset|^AMD Software|AMD Install Manager|NVIDIA|Realtek|^Intel\(R\)|Logi(?:tech| Options| Plugin)|G HUB|Razer|Corsair|SteelSeries|VB-?CABLE|Virtual Audio|Bluetooth|Synaptics|Elan|Dolby|DTS|Nahimic|Waves MaxxAudio")
});
static ANTICHEAT: LazyLock<Regex> = LazyLock::new(|| re(r"Vanguard|EasyAntiCheat|Easy Anti-Cheat|BattlEye|FACEIT|PunkBuster"));
static SECURITY: LazyLock<Regex> = LazyLock::new(|| re(r"McAfee|Norton|Avast|AVG |Kaspersky|Bitdefender|ESET|Malwarebytes|Sophos|Trend Micro"));
static GAMING: LazyLock<Regex> = LazyLock::new(|| {
    re(r"steam://|\bSteam\b|Valve|Epic Games|Riot|VALORANT|League of Legends|Ubisoft|Electronic Arts|\bEA app\b|Battle\.net|Blizzard|\bGOG\b|Rockstar|Roblox|Minecraft|Overwolf|CurseForge|Xbox|Bethesda|2K|Activision|Crosshair|GeForce|Adrenalin|MSI Afterburner|RivaTuner")
});
static OEM_BLOAT: LazyLock<Regex> = LazyLock::new(|| re(r"WildTangent|Booking\.com|ExpressVPN|Dropbox (?:Promotion|Offer)|Amazon Alexa|HP JumpStart|Dell SupportAssist Recovery|CyberLink .*Trial"));

pub fn classify_win32(name: &str, publisher: &str, uninstall: &str) -> Win32Class {
    let hay = format!("{name} {publisher}");
    let microsoft = publisher.to_ascii_lowercase().contains("microsoft");
    let gaming = GAMING.is_match(&hay) || GAMING.is_match(uninstall);
    let category = if gaming {
        Category::Gaming
    } else if microsoft {
        Category::Microsoft
    } else {
        Category::ThirdParty
    };
    let c = |rec, kind, note, preinstalled| Win32Class { category, rec, kind, note, preinstalled };

    if PROTECTED.is_match(name) {
        return c(R::Protected, K::System, "Required by Windows or by Blob Optimizers itself.", false);
    }
    if SECURITY.is_match(&hay) {
        return c(R::Caution, K::App, "Security software. After removal Microsoft Defender turns on automatically; use only one antivirus.", true);
    }
    if ANTICHEAT.is_match(&hay) {
        return c(R::Caution, K::Driver, "Anti-cheat driver. Online games that use it will not start until it is reinstalled.", false);
    }
    if RUNTIME.is_match(name) {
        return c(R::Caution, K::Runtime, "Shared runtime or developer component. Other programs and games may depend on it.", false);
    }
    if DRIVER.is_match(&hay) {
        return c(R::Caution, K::Driver, "Driver or device software. Removing it can disable hardware features (audio, network, peripherals, GPU settings).", false);
    }
    if OEM_BLOAT.is_match(&hay) {
        return c(R::Safe, K::App, "Promotional software preinstalled by the PC maker.", true);
    }
    if name.eq_ignore_ascii_case("Microsoft OneDrive") {
        return c(R::Caution, K::App, "Stops syncing. Files that exist only online stay in OneDrive; download any you need first.", true);
    }
    if name.starts_with("Microsoft 365 - ") || name.starts_with("Microsoft OneNote - ") {
        return c(R::Optional, K::App, "Preinstalled Office trial or language variant. Remove if you don't use Office.", true);
    }
    c(R::Optional, K::App, "Program you or another app installed. Remove it if you no longer use it.", false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appx_matching() {
        assert_eq!(appx_rule("Microsoft.BingNews").unwrap().rec, R::Safe);
        assert_eq!(appx_rule("Microsoft.MicrosoftEdge.Stable").unwrap().rec, R::Protected);
        assert_eq!(appx_rule("MicrosoftWindows.Client.WebExperience").unwrap().name, "Widgets");
        assert_eq!(appx_rule("MicrosoftWindows.Client.Core").unwrap().rec, R::Protected);
        assert!(appx_rule("Mozilla.Firefox").is_none());
        assert_eq!(appx_rule("Microsoft.Windows.Photos").unwrap().rec, R::Caution);
        assert_eq!(appx_rule("Microsoft.Windows.DevHome").unwrap().rec, R::Safe);
        assert_eq!(appx_rule("Microsoft.Windows.ShellExperienceHost").unwrap().rec, R::Protected);
        assert_eq!(derive_name("Microsoft.WindowsSoundRecorder"), "Windows Sound Recorder");
    }

    #[test]
    fn win32_classification() {
        assert_eq!(classify_win32("Microsoft Visual C++ 2012 Redistributable (x64)", "Microsoft Corporation", "").rec, R::Caution);
        assert_eq!(classify_win32("Riot Vanguard", "Riot Games, Inc.", "").kind, K::Driver);
        assert_eq!(classify_win32("Sons Of The Forest", "Endnight", "steam.exe steam://uninstall/1").category, C::Gaming);
        assert_eq!(classify_win32("TP-Link Archer TBE550E Driver", "TP-LINK", "").kind, K::Driver);
        assert_eq!(classify_win32("Microsoft Edge WebView2 Runtime", "Microsoft Corporation", "").rec, R::Protected);
        assert_eq!(classify_win32("Brave", "Brave Software Inc", "").rec, R::Optional);
        assert_eq!(classify_win32("Discord", "Discord Inc.", "").category, C::ThirdParty);
    }
}
