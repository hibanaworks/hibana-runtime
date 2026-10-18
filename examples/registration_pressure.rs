//! 256 different localside declarations with Rust's default recursion limit.
use core::{cell::Cell, convert::Infallible};
#[path = "support/wake_latch.rs"]
mod wake_latch;
use wake_latch::WakeLatch;

#[hibana_runtime::main(WakeLatch::new())]
async fn main() -> Result<(), Infallible> {
    let completed = Cell::new(0);
    {
        async {
            assert_eq!(completed.get(), 0);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 1);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 2);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 3);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 4);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 5);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 6);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 7);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 8);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 9);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 10);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 11);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 12);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 13);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 14);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 15);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 16);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 17);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 18);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 19);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 20);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 21);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 22);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 23);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 24);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 25);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 26);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 27);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 28);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 29);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 30);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 31);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 32);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 33);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 34);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 35);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 36);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 37);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 38);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 39);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 40);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 41);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 42);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 43);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 44);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 45);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 46);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 47);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 48);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 49);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 50);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 51);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 52);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 53);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 54);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 55);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 56);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 57);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 58);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 59);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 60);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 61);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 62);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 63);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 64);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 65);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 66);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 67);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 68);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 69);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 70);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 71);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 72);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 73);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 74);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 75);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 76);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 77);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 78);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 79);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 80);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 81);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 82);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 83);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 84);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 85);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 86);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 87);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 88);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 89);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 90);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 91);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 92);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 93);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 94);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 95);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 96);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 97);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 98);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 99);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 100);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 101);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 102);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 103);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 104);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 105);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 106);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 107);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 108);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 109);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 110);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 111);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 112);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 113);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 114);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 115);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 116);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 117);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 118);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 119);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 120);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 121);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 122);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 123);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 124);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 125);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 126);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 127);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 128);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 129);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 130);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 131);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 132);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 133);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 134);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 135);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 136);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 137);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 138);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 139);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 140);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 141);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 142);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 143);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 144);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 145);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 146);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 147);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 148);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 149);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 150);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 151);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 152);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 153);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 154);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 155);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 156);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 157);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 158);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 159);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 160);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 161);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 162);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 163);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 164);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 165);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 166);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 167);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 168);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 169);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 170);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 171);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 172);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 173);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 174);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 175);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 176);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 177);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 178);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 179);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 180);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 181);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 182);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 183);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 184);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 185);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 186);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 187);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 188);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 189);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 190);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 191);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 192);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 193);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 194);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 195);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 196);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 197);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 198);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 199);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 200);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 201);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 202);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 203);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 204);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 205);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 206);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 207);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 208);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 209);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 210);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 211);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 212);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 213);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 214);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 215);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 216);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 217);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 218);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 219);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 220);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 221);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 222);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 223);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 224);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 225);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 226);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 227);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 228);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 229);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 230);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 231);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 232);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 233);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 234);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 235);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 236);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 237);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 238);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 239);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 240);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 241);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 242);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 243);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 244);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 245);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 246);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 247);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 248);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 249);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 250);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 251);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 252);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 253);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 254);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
        async {
            assert_eq!(completed.get(), 255);
            completed.set(completed.get() + 1);
            Ok::<(), Infallible>(())
        };
    }
    assert_eq!(completed.get(), 256);
    Ok(())
}
