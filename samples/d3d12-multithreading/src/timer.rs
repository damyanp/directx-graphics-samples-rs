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
    start: Instant,
    now: Instant,
}

impl Default for Timer {
    fn default() -> Self {
        let now = Instant::now();
        Timer { start: now, now }        
    }
}

impl Timer {
    pub fn tick(&mut self) {
        self.now = Instant::now();
    }

    pub fn reset(&mut self) {
        self.tick();
        self.start = self.now;
    }

    pub fn get_elapsed(&self) -> Duration {
        self.now - self.start
    }
}
