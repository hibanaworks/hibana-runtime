//! 256 distinct named localsides, with Rust's default recursion limit.
use core::{cell::Cell, convert::Infallible};
#[path = "support/wake_latch.rs"]
mod wake_latch;
use wake_latch::WakeLatch;

async fn task<const SLOT: usize>(completed: &Cell<usize>) -> Result<(), Infallible> {
    assert_eq!(completed.get(), SLOT);
    completed.set(completed.get() + 1);
    Ok(())
}

#[hibana_runtime::main(WakeLatch::new())]
async fn main() -> Result<(), Infallible> {
    let completed = Cell::new(0);
    async {
        async { (task::<0>(&completed)).await };

        async { (task::<1>(&completed)).await };

        async { (task::<2>(&completed)).await };

        async { (task::<3>(&completed)).await };

        async { (task::<4>(&completed)).await };

        async { (task::<5>(&completed)).await };

        async { (task::<6>(&completed)).await };

        async { (task::<7>(&completed)).await };

        async { (task::<8>(&completed)).await };

        async { (task::<9>(&completed)).await };

        async { (task::<10>(&completed)).await };

        async { (task::<11>(&completed)).await };

        async { (task::<12>(&completed)).await };

        async { (task::<13>(&completed)).await };

        async { (task::<14>(&completed)).await };

        async { (task::<15>(&completed)).await };

        async { (task::<16>(&completed)).await };

        async { (task::<17>(&completed)).await };

        async { (task::<18>(&completed)).await };

        async { (task::<19>(&completed)).await };

        async { (task::<20>(&completed)).await };

        async { (task::<21>(&completed)).await };

        async { (task::<22>(&completed)).await };

        async { (task::<23>(&completed)).await };

        async { (task::<24>(&completed)).await };

        async { (task::<25>(&completed)).await };

        async { (task::<26>(&completed)).await };

        async { (task::<27>(&completed)).await };

        async { (task::<28>(&completed)).await };

        async { (task::<29>(&completed)).await };

        async { (task::<30>(&completed)).await };

        async { (task::<31>(&completed)).await };

        async { (task::<32>(&completed)).await };

        async { (task::<33>(&completed)).await };

        async { (task::<34>(&completed)).await };

        async { (task::<35>(&completed)).await };

        async { (task::<36>(&completed)).await };

        async { (task::<37>(&completed)).await };

        async { (task::<38>(&completed)).await };

        async { (task::<39>(&completed)).await };

        async { (task::<40>(&completed)).await };

        async { (task::<41>(&completed)).await };

        async { (task::<42>(&completed)).await };

        async { (task::<43>(&completed)).await };

        async { (task::<44>(&completed)).await };

        async { (task::<45>(&completed)).await };

        async { (task::<46>(&completed)).await };

        async { (task::<47>(&completed)).await };

        async { (task::<48>(&completed)).await };

        async { (task::<49>(&completed)).await };

        async { (task::<50>(&completed)).await };

        async { (task::<51>(&completed)).await };

        async { (task::<52>(&completed)).await };

        async { (task::<53>(&completed)).await };

        async { (task::<54>(&completed)).await };

        async { (task::<55>(&completed)).await };

        async { (task::<56>(&completed)).await };

        async { (task::<57>(&completed)).await };

        async { (task::<58>(&completed)).await };

        async { (task::<59>(&completed)).await };

        async { (task::<60>(&completed)).await };

        async { (task::<61>(&completed)).await };

        async { (task::<62>(&completed)).await };

        async { (task::<63>(&completed)).await };

        async { (task::<64>(&completed)).await };

        async { (task::<65>(&completed)).await };

        async { (task::<66>(&completed)).await };

        async { (task::<67>(&completed)).await };

        async { (task::<68>(&completed)).await };

        async { (task::<69>(&completed)).await };

        async { (task::<70>(&completed)).await };

        async { (task::<71>(&completed)).await };

        async { (task::<72>(&completed)).await };

        async { (task::<73>(&completed)).await };

        async { (task::<74>(&completed)).await };

        async { (task::<75>(&completed)).await };

        async { (task::<76>(&completed)).await };

        async { (task::<77>(&completed)).await };

        async { (task::<78>(&completed)).await };

        async { (task::<79>(&completed)).await };

        async { (task::<80>(&completed)).await };

        async { (task::<81>(&completed)).await };

        async { (task::<82>(&completed)).await };

        async { (task::<83>(&completed)).await };

        async { (task::<84>(&completed)).await };

        async { (task::<85>(&completed)).await };

        async { (task::<86>(&completed)).await };

        async { (task::<87>(&completed)).await };

        async { (task::<88>(&completed)).await };

        async { (task::<89>(&completed)).await };

        async { (task::<90>(&completed)).await };

        async { (task::<91>(&completed)).await };

        async { (task::<92>(&completed)).await };

        async { (task::<93>(&completed)).await };

        async { (task::<94>(&completed)).await };

        async { (task::<95>(&completed)).await };

        async { (task::<96>(&completed)).await };

        async { (task::<97>(&completed)).await };

        async { (task::<98>(&completed)).await };

        async { (task::<99>(&completed)).await };

        async { (task::<100>(&completed)).await };

        async { (task::<101>(&completed)).await };

        async { (task::<102>(&completed)).await };

        async { (task::<103>(&completed)).await };

        async { (task::<104>(&completed)).await };

        async { (task::<105>(&completed)).await };

        async { (task::<106>(&completed)).await };

        async { (task::<107>(&completed)).await };

        async { (task::<108>(&completed)).await };

        async { (task::<109>(&completed)).await };

        async { (task::<110>(&completed)).await };

        async { (task::<111>(&completed)).await };

        async { (task::<112>(&completed)).await };

        async { (task::<113>(&completed)).await };

        async { (task::<114>(&completed)).await };

        async { (task::<115>(&completed)).await };

        async { (task::<116>(&completed)).await };

        async { (task::<117>(&completed)).await };

        async { (task::<118>(&completed)).await };

        async { (task::<119>(&completed)).await };

        async { (task::<120>(&completed)).await };

        async { (task::<121>(&completed)).await };

        async { (task::<122>(&completed)).await };

        async { (task::<123>(&completed)).await };

        async { (task::<124>(&completed)).await };

        async { (task::<125>(&completed)).await };

        async { (task::<126>(&completed)).await };

        async { (task::<127>(&completed)).await };

        async { (task::<128>(&completed)).await };

        async { (task::<129>(&completed)).await };

        async { (task::<130>(&completed)).await };

        async { (task::<131>(&completed)).await };

        async { (task::<132>(&completed)).await };

        async { (task::<133>(&completed)).await };

        async { (task::<134>(&completed)).await };

        async { (task::<135>(&completed)).await };

        async { (task::<136>(&completed)).await };

        async { (task::<137>(&completed)).await };

        async { (task::<138>(&completed)).await };

        async { (task::<139>(&completed)).await };

        async { (task::<140>(&completed)).await };

        async { (task::<141>(&completed)).await };

        async { (task::<142>(&completed)).await };

        async { (task::<143>(&completed)).await };

        async { (task::<144>(&completed)).await };

        async { (task::<145>(&completed)).await };

        async { (task::<146>(&completed)).await };

        async { (task::<147>(&completed)).await };

        async { (task::<148>(&completed)).await };

        async { (task::<149>(&completed)).await };

        async { (task::<150>(&completed)).await };

        async { (task::<151>(&completed)).await };

        async { (task::<152>(&completed)).await };

        async { (task::<153>(&completed)).await };

        async { (task::<154>(&completed)).await };

        async { (task::<155>(&completed)).await };

        async { (task::<156>(&completed)).await };

        async { (task::<157>(&completed)).await };

        async { (task::<158>(&completed)).await };

        async { (task::<159>(&completed)).await };

        async { (task::<160>(&completed)).await };

        async { (task::<161>(&completed)).await };

        async { (task::<162>(&completed)).await };

        async { (task::<163>(&completed)).await };

        async { (task::<164>(&completed)).await };

        async { (task::<165>(&completed)).await };

        async { (task::<166>(&completed)).await };

        async { (task::<167>(&completed)).await };

        async { (task::<168>(&completed)).await };

        async { (task::<169>(&completed)).await };

        async { (task::<170>(&completed)).await };

        async { (task::<171>(&completed)).await };

        async { (task::<172>(&completed)).await };

        async { (task::<173>(&completed)).await };

        async { (task::<174>(&completed)).await };

        async { (task::<175>(&completed)).await };

        async { (task::<176>(&completed)).await };

        async { (task::<177>(&completed)).await };

        async { (task::<178>(&completed)).await };

        async { (task::<179>(&completed)).await };

        async { (task::<180>(&completed)).await };

        async { (task::<181>(&completed)).await };

        async { (task::<182>(&completed)).await };

        async { (task::<183>(&completed)).await };

        async { (task::<184>(&completed)).await };

        async { (task::<185>(&completed)).await };

        async { (task::<186>(&completed)).await };

        async { (task::<187>(&completed)).await };

        async { (task::<188>(&completed)).await };

        async { (task::<189>(&completed)).await };

        async { (task::<190>(&completed)).await };

        async { (task::<191>(&completed)).await };

        async { (task::<192>(&completed)).await };

        async { (task::<193>(&completed)).await };

        async { (task::<194>(&completed)).await };

        async { (task::<195>(&completed)).await };

        async { (task::<196>(&completed)).await };

        async { (task::<197>(&completed)).await };

        async { (task::<198>(&completed)).await };

        async { (task::<199>(&completed)).await };

        async { (task::<200>(&completed)).await };

        async { (task::<201>(&completed)).await };

        async { (task::<202>(&completed)).await };

        async { (task::<203>(&completed)).await };

        async { (task::<204>(&completed)).await };

        async { (task::<205>(&completed)).await };

        async { (task::<206>(&completed)).await };

        async { (task::<207>(&completed)).await };

        async { (task::<208>(&completed)).await };

        async { (task::<209>(&completed)).await };

        async { (task::<210>(&completed)).await };

        async { (task::<211>(&completed)).await };

        async { (task::<212>(&completed)).await };

        async { (task::<213>(&completed)).await };

        async { (task::<214>(&completed)).await };

        async { (task::<215>(&completed)).await };

        async { (task::<216>(&completed)).await };

        async { (task::<217>(&completed)).await };

        async { (task::<218>(&completed)).await };

        async { (task::<219>(&completed)).await };

        async { (task::<220>(&completed)).await };

        async { (task::<221>(&completed)).await };

        async { (task::<222>(&completed)).await };

        async { (task::<223>(&completed)).await };

        async { (task::<224>(&completed)).await };

        async { (task::<225>(&completed)).await };

        async { (task::<226>(&completed)).await };

        async { (task::<227>(&completed)).await };

        async { (task::<228>(&completed)).await };

        async { (task::<229>(&completed)).await };

        async { (task::<230>(&completed)).await };

        async { (task::<231>(&completed)).await };

        async { (task::<232>(&completed)).await };

        async { (task::<233>(&completed)).await };

        async { (task::<234>(&completed)).await };

        async { (task::<235>(&completed)).await };

        async { (task::<236>(&completed)).await };

        async { (task::<237>(&completed)).await };

        async { (task::<238>(&completed)).await };

        async { (task::<239>(&completed)).await };

        async { (task::<240>(&completed)).await };

        async { (task::<241>(&completed)).await };

        async { (task::<242>(&completed)).await };

        async { (task::<243>(&completed)).await };

        async { (task::<244>(&completed)).await };

        async { (task::<245>(&completed)).await };

        async { (task::<246>(&completed)).await };

        async { (task::<247>(&completed)).await };

        async { (task::<248>(&completed)).await };

        async { (task::<249>(&completed)).await };

        async { (task::<250>(&completed)).await };

        async { (task::<251>(&completed)).await };

        async { (task::<252>(&completed)).await };

        async { (task::<253>(&completed)).await };

        async { (task::<254>(&completed)).await };

        async { (task::<255>(&completed)).await };
        Ok(())
    }
    .await?;
    assert_eq!(completed.get(), 256);
    Ok(())
}
