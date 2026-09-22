//! 涌现行为检测
//! 
//! 群体智能的核心：局部规则 → 全局涌现

#[derive(Debug, Clone)]
pub struct EmergenceDetector {
    /// 历史指标
    history: Vec<(u64, f64, f64)>, // (timestamp, throughput, avg_latency)
    /// 涌现阈值
    emergence_threshold: f64,
    /// 检测窗口
    window_size: usize,
}

#[derive(Debug, Clone)]
pub struct EmergenceSignal {
    pub signal_type: EmergenceType,
    pub strength: f64,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmergenceType {
    /// 协同涌现：多个节点自发协作
    Collaboration,
    /// 负载均衡涌现：流量自然分布
    LoadBalancing,
    /// 容错涌现：节点故障后自动恢复
    FaultTolerance,
    /// 进化涌现：整体能力提升
    Evolution,
}

impl EmergenceDetector {
    pub fn new(threshold: f64, window_size: usize) -> Self {
        Self {
            history: Vec::new(),
            emergence_threshold: threshold,
            window_size,
        }
    }

    pub fn record(&mut self, timestamp: u64, throughput: f64, latency: f64) {
        self.history.push((timestamp, throughput, latency));
        if self.history.len() > self.window_size * 2 {
            let drain_count = self.history.len() - self.window_size;
            self.history.drain(0..drain_count);
        }
    }

    /// 检测涌现信号
    pub fn detect(&self) -> Vec<EmergenceSignal> {
        let mut signals = Vec::new();
        
        if self.history.len() < self.window_size {
            return signals;
        }

        let recent: Vec<_> = self.history.iter().rev().take(self.window_size).collect();
        let older: Vec<_> = self.history.iter()
            .rev()
            .skip(self.window_size)
            .take(self.window_size)
            .collect();

        if older.is_empty() {
            return signals;
        }

        // 检测吞吐量增长
        let recent_throughput: f64 = recent.iter().map(|(_, t, _)| t).sum::<f64>() / recent.len() as f64;
        let older_throughput: f64 = older.iter().map(|(_, t, _)| t).sum::<f64>() / older.len() as f64;
        
        if older_throughput > 0.0 {
            let growth = (recent_throughput - older_throughput) / older_throughput;
            if growth > self.emergence_threshold {
                signals.push(EmergenceSignal {
                    signal_type: EmergenceType::Collaboration,
                    strength: growth,
                    description: format!("吞吐量增长 {:.1}%，检测到协同涌现", growth * 100.0),
                });
            }
        }

        // 检测延迟下降
        let recent_latency: f64 = recent.iter().map(|(_, _, l)| l).sum::<f64>() / recent.len() as f64;
        let older_latency: f64 = older.iter().map(|(_, _, l)| l).sum::<f64>() / older.len() as f64;
        
        if older_latency > 0.0 {
            let improvement = (older_latency - recent_latency) / older_latency;
            if improvement > self.emergence_threshold {
                signals.push(EmergenceSignal {
                    signal_type: EmergenceType::LoadBalancing,
                    strength: improvement,
                    description: format!("延迟下降 {:.1}%，检测到负载均衡涌现", improvement * 100.0),
                });
            }
        }

        signals
    }
}
