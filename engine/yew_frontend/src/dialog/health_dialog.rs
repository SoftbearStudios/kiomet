// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::dialog::dialog::Dialog;
//use crate::translation::{use_translation, Translation};
use crate::component::link::Link;
use stylist::yew::styled_component;
use yew::{function_component, html, Children, Html, Properties};
use yew_icons::{Icon, IconId};

#[styled_component(HealthDialog)]
pub fn health_dialog() -> Html {
    //let t = use_translation();

    let style = css!(
        r#"
        ul {
            padding-inline-start: 2rem;
        }
    "#
    );

    html! {
        <Dialog title={"Health"}>
            <p>
                {"At Softbear Games, we're dedicated to promoting healthy practices for gamers. "}
                {"This experimental page offers tips and product suggestions backed by trustworthy sources. "}
                {"For personalized advice, please consult your doctor. "}
                {"When applicable, we suggest products via Amazon affiliate links. As an Amazon Associate we earn
                from qualifying purchases."}
            </p>

            <div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 1rem;" class={style}>
                <Card
                    icon_id={IconId::FontAwesomeSolidPersonRunning}
                    title="Exercise"
                    source="https://www.mayoclinic.org/healthy-lifestyle/fitness/expert-answers/exercise/faq-2005791630"
                >
                    <ul>
                        <li>{"Be physically active at least 30 minutes per day, with moderate intensity"}</li>
                        <li>
                            {"Try walking, jogging, or biking (with a "}
                            <Link href="https://www.amazon.com/gp/search?ie=UTF8&tag=softbear-20&linkCode=ur2&linkId=dc6788afb8dda053edf52d1c86579a19&camp=1789&creative=9325&index=aps&keywords=bike helmet">
                                {"helmet"}
                            </Link>
                            {")"}
                        </li>
                        <li>{"Limit time spent sitting down"}</li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::BootstrapMoonStarsFill}
                    title="Sleep"
                    source={"https://www.sleepfoundation.org/how-sleep-works/how-much-sleep-do-we-really-need"}
                >
                    <ul>
                        <li>{"Get 8 hours of sleep per day"}</li>
                        <li>{"Limit sound and blue light from screens"}</li>
                        <li>{"Aim for consistency"}</li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::FontAwesomeSolidGlassWaterDroplet}
                    title="Hydration"
                    source="https://www.mayoclinic.org/healthy-lifestyle/nutrition-and-healthy-eating/in-depth/water/art-20044256"
                >
                    <ul>
                        <li>{"Figure out how much water you need (~8 cups per day)"}</li>
                        <li>{"Pay attention to symptoms of dehydration"}</li>
                        <li>
                            {"Get a reusable "}
                            <Link href="https://www.amazon.com/gp/search?ie=UTF8&tag=softbear-20&linkCode=ur2&linkId=4ba39739333e769c1c5893e24cf12699&camp=1789&creative=9325&index=aps&keywords=reusable water bottle">
                                {"bottle"}
                            </Link>
                            {" or "}
                            <Link href="https://www.amazon.com/YETI-Stackable-Insulated-Stainless-MagSlider/dp/B0BTTXCNCY/?&_encoding=UTF8&tag=softbear-20&linkCode=ur2&linkId=53e2b1f96e94312e85f2d3984abd79aa&camp=1789&creative=9325">
                                {"mug"}
                            </Link>
                        </li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::FontAwesomeSolidScaleBalanced}
                    title="Balance"
                    source="https://www.ditchthelabel.org/7-ways-to-find-a-healthy-gaming-life-balance"
                >
                    <ul>
                        <li>{"Limit video games to a few hours a week"}</li>
                        <li>{"Prioritize life, school, and work"}</li>
                        <li>{"Find time for social activities and hobbies"}</li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::FontAwesomeSolidBowlFood}
                    title="Nutrition"
                    source="https://www.cdc.gov/nccdphp/dnpao/features/healthy-eating-tips/index.html"
                >
                    <ul>
                        <li>{"Plan a balanced diet"}</li>
                        <li>{"Eat fruits, vegetables, whole grains, and protein"}</li>
                        <li>{"Avoid excess sugar and processed foods"}</li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::FontAwesomeSolidSquarePersonConfined}
                    title="Stress"
                    source="https://www.heart.org/en/healthy-living/healthy-lifestyle/stress-management/3-tips-to-manage-stress"
                >
                    <ul>
                        <li>{"Develop healthy coping mechanisms"}</li>
                        <li>{"Exercise and finding balance can help"}</li>
                        <li>{"Be proactive about things within your control"}</li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::FontAwesomeSolidPersonBurst}
                    title="Skincare"
                    source="https://www.aad.org/public/everyday-care/skin-care-secrets/routine/healthier-looking-skin"
                >
                    <ul>
                        <li>
                            {"Apply "}
                            <Link href="https://www.amazon.com/gp/product/B07Y9TT1K8/?&_encoding=UTF8&tag=softbear-20&linkCode=ur2&linkId=42eec23aac05a98db0c3e7c515868eb5&camp=1789&creative=9325">
                                {"sunscreen"}
                            </Link>
                            {" regularly"}
                        </li>
                        <li>
                            {"Consider wearing a "}
                            <Link href="https://www.amazon.com/gp/search?ie=UTF8&tag=softbear-20&linkCode=ur2&linkId=ab37128855087c40c762710f1eaaa86f&camp=1789&creative=9325&index=aps&keywords=wide brim hat">
                                {"wide brim hat"}
                            </Link>
                        </li>
                        <li>{"Avoid touching your face"}</li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::FontAwesomeSolidTooth}
                    title="Teeth"
                    source="https://www.webmd.com/oral-health/tooth-decay-prevention"
                >
                    <ul>
                        <li>{"Tooth decay is preventable, especially with regular professional cleaning"}</li>
                        <li>
                            <Link href="https://www.amazon.com/gp/product/B07P7QRCWB/?&_encoding=UTF8&tag=softbear-20&linkCode=ur2&linkId=e3e6827e7f4976a05bcf330bbf8799cd&camp=1789&creative=9325">
                                {"Brush your teeth"}
                            </Link>
                            {" twice a day, ideally after each meal"}</li>
                        <li>
                            {"Make sure to "}
                            <Link href="https://www.amazon.com/gp/product/B09GXPCRX1?&_encoding=UTF8&tag=softbear-20&linkCode=ur2&linkId=eb9e9be6bac619219ff8517027149a3d&camp=1789&creative=9325">
                                {"floss"}
                            </Link>
                            {" daily"}
                        </li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::FontAwesomeSolidEye}
                    title="Vision"
                    source="https://www.nei.nih.gov/learn-about-eye-health/nei-for-kids/healthy-vision-tips"
                >
                    <ul>
                        <li>
                            {"Wear protective eyewear, especially "}
                            <Link href="https://www.amazon.com/gp/search?ie=UTF8&tag=softbear-20&linkCode=ur2&linkId=3ff97dfb14a6e553c2c386d1939b2942&camp=1789&creative=9325&index=aps&keywords=sunglasses">
                                {"sunglasses"}
                            </Link>
                        </li>
                        <li>{"Regularly glance far away to relax your eyes"}</li>
                        <li>{"Get you eyes checked by a professional"}</li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::FontAwesomeSolidVirusCovid}
                    title={"Germs"}
                    source={"https://news.llu.edu/patient-care/5-ways-keep-germ-free-home-and-work"}
                >
                    <ul>
                        <li>{"Avoid close contact involving contagious disease"}</li>
                        <li>
                            {"Wash hands using "}
                            <Link href="https://www.amazon.com/gp/search?ie=UTF8&tag=softbear-20&linkCode=ur2&linkId=2fbb5414987d8ae2449dc74c81a0a84d&camp=1789&creative=9325&index=aps&keywords=environmentally friendly hand soap">
                                {"soap"}
                            </Link>
                            {" and water"}
                        </li>
                        <li>{"Cover your coughs"}</li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::FontAwesomeSolidLungs}
                    title="Substances"
                    source={"https://www.samhsa.gov/adult-drug-use"}
                >
                    <ul>
                        <li>{"Understand the risks and side-effects"} </li>
                        <li>{"Avoid forming a substance addiction"}</li>
                        <li>{"Explore all available resources if someone is struggling with an addiction"}</li>
                    </ul>
                </Card>
                <Card
                    icon_id={IconId::LucidePhoneCall}
                    title="Emergencies"
                    source={"https://www.redcross.org/get-help/how-to-prepare-for-emergencies.html"}
                >
                    <ul>
                        <li>
                            {"Make a plan and gather "}
                            <Link href="https://www.amazon.com/gp/search?ie=UTF8&tag=softbear-20&linkCode=ur2&linkId=90fe90e990bba9b529bb135d248225e8&camp=1789&creative=9325&index=aps&keywords=flash light">
                                {"supplies"}
                            </Link>
                        </li>
                        <li>{"Research the most likely disasters in your area"}</li>
                        <li>{"Learn how to do CPR"}</li>
                    </ul>
                </Card>
            </div>
        </Dialog>
    }
}

#[derive(Properties, PartialEq)]
struct CardProps {
    icon_id: IconId,
    title: &'static str,
    source: Option<&'static str>,
    children: Children,
}

#[function_component(Card)]
fn card(props: &CardProps) -> Html {
    html! {
        <div style="background-color: #2b5c8d; padding: 0.5rem; border-radius: 0.5rem; position: relative;">
            <h3 style="margin: 0;">
                <Icon
                    icon_id={props.icon_id}
                    width={"1.2rem"}
                    height={"1.4rem"}
                    style={"vertical-align: bottom;"}
                />
                {" "}
                {props.title}
            </h3>
            <div style="margin: 0; margin-top: 0.5rem;">{props.children.clone()}</div>
            if let Some(href) = props.source {
                <div style="position: absolute; bottom: 0.4rem; right: 0.5rem; filter: opacity(0.6);">
                    <Link {href}>{"Learn more"}</Link>
                </div>
            }
        </div>
    }
}
