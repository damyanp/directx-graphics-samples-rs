//*********************************************************
//
// Copyright (c) Microsoft. All rights reserved.
// This code is licensed under the MIT License (MIT).
// THIS CODE IS PROVIDED *AS IS* WITHOUT WARRANTY OF
// ANY KIND, EITHER EXPRESS OR IMPLIED, INCLUDING ANY
// IMPLIED WARRANTIES OF FITNESS FOR A PARTICULAR
// PURPOSE, MERCHANTABILITY, OR NON-INFRINGEMENT.
//
//*********************************************************

use std::time::{Duration, Instant};

pub struct Timer {
    last_time: Instant,
    now: Instant,
}

impl Default for Timer {
    fn default() -> Self {
        let now = Instant::now();
        Timer {
            last_time: now,
            now,
        }
    }
}

impl Timer {
    pub fn tick(&mut self) {
        self.last_time = self.now;
        self.now = Instant::now();
    }

    pub fn _reset(&mut self) {
        self.tick();
        self.last_time = self.now;
    }

    pub fn get_elapsed(&self) -> Duration {
        self.now - self.last_time
    }
}
