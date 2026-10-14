use leptos::prelude::*;

struct EloTier {
    min: i64,
    name: &'static str,
    rank: &'static str,
    bg_gradient_dark: &'static str,
    border_color: &'static str,
    text_color_minimal: &'static str,
}

const ELO_TIERS: &[EloTier] = &[
    EloTier { min: 0,    name: "Bronze",      rank: "I",  bg_gradient_dark: "from-amber-800 to-amber-900",   border_color: "border-amber-600/60",  text_color_minimal: "text-amber-400" },
    EloTier { min: 1000, name: "Silver",      rank: "I",  bg_gradient_dark: "from-slate-600 to-slate-700",   border_color: "border-slate-400/60",  text_color_minimal: "text-slate-300" },
    EloTier { min: 1200, name: "Silver",      rank: "II", bg_gradient_dark: "from-slate-600 to-slate-700",   border_color: "border-slate-400/60",  text_color_minimal: "text-slate-300" },
    EloTier { min: 1400, name: "Gold",        rank: "I",  bg_gradient_dark: "from-yellow-600 to-yellow-700", border_color: "border-yellow-400/60", text_color_minimal: "text-yellow-300" },
    EloTier { min: 1600, name: "Gold",        rank: "II", bg_gradient_dark: "from-yellow-600 to-yellow-700", border_color: "border-yellow-400/60", text_color_minimal: "text-yellow-300" },
    EloTier { min: 1800, name: "Platinum",    rank: "I",  bg_gradient_dark: "from-teal-600 to-teal-700",     border_color: "border-teal-400/60",   text_color_minimal: "text-teal-300" },
    EloTier { min: 2000, name: "Platinum",    rank: "II", bg_gradient_dark: "from-teal-600 to-teal-700",     border_color: "border-teal-400/60",   text_color_minimal: "text-teal-300" },
    EloTier { min: 2200, name: "Diamond",     rank: "I",  bg_gradient_dark: "from-blue-600 to-blue-700",     border_color: "border-blue-400/60",   text_color_minimal: "text-blue-300" },
    EloTier { min: 2400, name: "Diamond",     rank: "II", bg_gradient_dark: "from-blue-600 to-blue-700",     border_color: "border-blue-400/60",   text_color_minimal: "text-blue-300" },
    EloTier { min: 2600, name: "Master",      rank: "I",  bg_gradient_dark: "from-purple-700 to-purple-800", border_color: "border-purple-500/60", text_color_minimal: "text-purple-300" },
    EloTier { min: 2800, name: "Master",      rank: "II", bg_gradient_dark: "from-purple-700 to-purple-800", border_color: "border-purple-500/60", text_color_minimal: "text-purple-300" },
    EloTier { min: 3000, name: "Grandmaster", rank: "",   bg_gradient_dark: "from-pink-700 to-pink-800",     border_color: "border-pink-500/60",   text_color_minimal: "text-pink-300" },
];

fn tier_for(elo: i64) -> &'static EloTier {
    let v = elo.clamp(100, 10000);
    ELO_TIERS
        .iter()
        .rev()
        .find(|t| v >= t.min)
        .unwrap_or(&ELO_TIERS[0])
}

#[component]
pub fn EloBadge(
    elo: i64,
    #[prop(default = "sm")] size: &'static str,
    #[prop(default = "compact")] variant: &'static str,
    #[prop(optional)] elo_change: Option<i64>,
) -> impl IntoView {
    let tier = tier_for(elo);
    let (text_size, number_size, padding) = match size {
        "md" => ("text-sm", "text-xs", "px-2 py-1"),
        "lg" => ("text-base", "text-sm", "px-3 py-1.5"),
        _ => ("text-xs", "text-[10px]", "px-1.5 py-0.5"),
    };
    let title = if tier.rank.is_empty() {
        format!("{} - {} ELO", tier.name, elo)
    } else {
        format!("{} {} - {} ELO", tier.name, tier.rank, elo)
    };

    match variant {
        "minimal" => view! {
            <div class=format!("inline-flex items-center gap-1 {}", tier.text_color_minimal)
                title=title role="status">
                <span class=format!("{number_size} font-bold")>{elo}</span>
            </div>
        }
        .into_any(),
        "default" => view! {
            <div class=format!(
                "inline-flex items-center gap-1.5 rounded-lg border font-semibold shadow-md transition-all \
                 bg-gradient-to-r {} {} {}",
                tier.bg_gradient_dark, padding, tier.border_color,
            ) title=title role="status">
                <span class=format!("{text_size} font-bold text-white drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)]")>
                    {tier.name} " " {tier.rank}
                </span>
                <span class=format!("{number_size} font-extrabold text-white drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)]")>
                    {elo}
                </span>
                {elo_change.map(|c| {
                    let cls = if c > 0 { "text-green-100" } else if c < 0 { "text-red-100" } else { "text-white/90" };
                    view! {
                        <span class=format!("{number_size} font-bold drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)] {cls}")>
                            {if c > 0 { format!("+{c}") } else { c.to_string() }}
                        </span>
                    }
                })}
            </div>
        }
        .into_any(),
        _ => view! {
            <div class=format!(
                "inline-flex items-center justify-center rounded-md border font-bold shadow-sm \
                 bg-gradient-to-br {} {} {}",
                tier.bg_gradient_dark, padding, tier.border_color,
            ) title=title role="status">
                <span class=format!("{number_size} font-extrabold text-white drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)]")>
                    {elo}
                </span>
            </div>
        }
        .into_any(),
    }
}
