use crate::error::Result;
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::sleep;

/// Relay pin configuration.
/// Default: BCM GPIO 17. Configure via POWER_CONTROLLER_GPIO_PIN env var.
pub const DEFAULT_GPIO_PIN: u8 = 17;

/// Trait abstracting relay operations for testability.
#[async_trait]
pub trait RelayTrait: Send + Sync {
    async fn short_press(&mut self) -> Result<()>;
    async fn long_press(&mut self) -> Result<()>;
    async fn hard_reset(&mut self) -> Result<()>;
}

/// Relay control abstraction.
/// The relay is active-low: setting the pin LOW closes the relay (shorts the PWR pins).
#[cfg(target_os = "linux")]
pub struct Relay {
    pin: Option<rppal::gpio::OutputPin>,
}

/// Stub relay for non-Linux targets (development/testing on macOS).
#[cfg(not(target_os = "linux"))]
pub struct Relay;

/// Mock relay for unit testing — instant, no GPIO, no sleeps.
#[cfg(test)]
pub struct MockRelay {
    /// Shared call counter — shared between relay and test state.
    pub calls: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, usize>>>,
}

#[cfg(test)]
impl MockRelay {
    pub fn new() -> Self {
        Self {
            calls: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    fn record(&self, action: &str) {
        let mut map = self.calls.lock().unwrap();
        *map.entry(action.to_string()).or_insert(0) += 1;
    }
}

#[cfg(test)]
impl Clone for MockRelay {
    fn clone(&self) -> Self {
        Self {
            calls: self.calls.clone(),
        }
    }
}

#[cfg(test)]
impl Default for MockRelay {
    fn default() -> Self {
        Self::new()
    }
}

// ── Real Relay (Linux) ──────────────────────────────────────────────

#[cfg(target_os = "linux")]
impl Relay {
    /// Initialize the relay on the specified GPIO pin.
    pub fn new(pin_number: Option<u8>) -> Result<Self> {
        let pin_num = pin_number.unwrap_or(DEFAULT_GPIO_PIN);
        let gpio = rppal::gpio::Gpio::new()
            .map_err(|e| AppError::GpioSetup(format!("Failed to initialize GPIO: {e}")))?;
        let pin = gpio
            .get(pin_num)
            .map_err(|e| AppError::GpioSetup(format!("Failed to get GPIO pin {pin_num}: {e}")))?
            .into_output();

        Ok(Self { pin: Some(pin) })
    }

    async fn activate(&mut self) -> Result<()> {
        if let Some(ref mut pin) = self.pin {
            pin.set_low();
        }
        Ok(())
    }

    async fn deactivate(&mut self) -> Result<()> {
        if let Some(ref mut pin) = self.pin {
            pin.set_high();
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
#[async_trait]
impl RelayTrait for Relay {
    async fn short_press(&mut self) -> Result<()> {
        self.activate().await?;
        sleep(Duration::from_millis(500)).await;
        self.deactivate().await?;
        Ok(())
    }

    async fn long_press(&mut self) -> Result<()> {
        self.activate().await?;
        sleep(Duration::from_secs(5)).await;
        self.deactivate().await?;
        Ok(())
    }

    async fn hard_reset(&mut self) -> Result<()> {
        self.activate().await?;
        sleep(Duration::from_secs(5)).await;
        self.deactivate().await?;
        sleep(Duration::from_secs(2)).await;
        self.activate().await?;
        sleep(Duration::from_millis(500)).await;
        self.deactivate().await?;
        Ok(())
    }
}

// ── Stub Relay (non-Linux / macOS) ──────────────────────────────────

#[cfg(not(target_os = "linux"))]
impl Relay {
    /// Create a simulation relay (no hardware control, logs actions instead).
    pub fn new(_pin_number: Option<u8>) -> Result<Self> {
        println!("⚠  Running in SIMULATION mode (no GPIO control available on this platform).");
        println!(
            "   Set POWER_CONTROLLER_GPIO_PIN to a valid BCM pin on Linux to enable hardware."
        );
        Ok(Self)
    }
}

#[cfg(not(target_os = "linux"))]
#[async_trait]
impl RelayTrait for Relay {
    async fn short_press(&mut self) -> Result<()> {
        println!("  [SIM] Short press (0.5s)");
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    async fn long_press(&mut self) -> Result<()> {
        println!("  [SIM] Long press (5s)");
        sleep(Duration::from_secs(5)).await;
        Ok(())
    }

    async fn hard_reset(&mut self) -> Result<()> {
        println!("  [SIM] Hard reset (5s + 2s pause + 0.5s)");
        sleep(Duration::from_secs(5)).await;
        sleep(Duration::from_secs(2)).await;
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }
}

// ── Mock Relay (tests) ──────────────────────────────────────────────

#[cfg(test)]
#[async_trait]
impl RelayTrait for MockRelay {
    async fn short_press(&mut self) -> Result<()> {
        self.record("short_press");
        Ok(())
    }

    async fn long_press(&mut self) -> Result<()> {
        self.record("long_press");
        Ok(())
    }

    async fn hard_reset(&mut self) -> Result<()> {
        self.record("hard_reset");
        Ok(())
    }
}

#[cfg(test)]
impl MockRelay {
    /// Get the call count for a specific action.
    pub fn call_count(&self, action: &str) -> usize {
        self.calls.lock().unwrap().get(action).copied().unwrap_or(0)
    }

    /// Reset all call counters.
    pub fn reset_counts(&self) {
        self.calls.lock().unwrap().clear();
    }
}
