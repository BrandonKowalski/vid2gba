pub fn dist<const N: usize>(a: &[f32; N], b: &[f32; N]) -> f32 {
    let mut s = 0.0;
    for i in 0..N {
        let d = a[i] - b[i];
        s += d * d;
    }
    s
}

pub fn nearest<const N: usize>(centroids: &[[f32; N]], v: &[f32; N]) -> usize {
    let mut best = 0;
    let mut best_d = f32::MAX;
    for (i, c) in centroids.iter().enumerate() {
        let d = dist(c, v);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

pub fn kmeans<const N: usize>(data: &[[f32; N]], k: usize, iters: usize) -> Vec<[f32; N]> {
    let n = data.len();
    let k = k.min(n);
    if k == 0 {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| data[a].iter().sum::<f32>().total_cmp(&data[b].iter().sum::<f32>()));
    let mut centroids: Vec<[f32; N]> = (0..k).map(|j| data[order[(2 * j + 1) * n / (2 * k)]]).collect();
    for _ in 0..iters {
        let mut sums = vec![[0f32; N]; k];
        let mut counts = vec![0u32; k];
        for v in data {
            let c = nearest(&centroids, v);
            counts[c] += 1;
            for d in 0..N {
                sums[c][d] += v[d];
            }
        }
        for c in 0..k {
            if counts[c] > 0 {
                for d in 0..N {
                    centroids[c][d] = sums[c][d] / counts[c] as f32;
                }
            }
        }
    }
    centroids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_two_clusters() {
        let mut data = Vec::new();
        for i in 0..50 {
            data.push([10.0 + (i % 3) as f32, 10.0]);
            data.push([200.0 + (i % 3) as f32, 50.0]);
        }
        let mut c = kmeans(&data, 2, 10);
        c.sort_by(|a, b| a[0].total_cmp(&b[0]));
        assert!((c[0][0] - 11.0).abs() < 1.0 && (c[0][1] - 10.0).abs() < 1.0);
        assert!((c[1][0] - 201.0).abs() < 1.0 && (c[1][1] - 50.0).abs() < 1.0);
    }

    #[test]
    fn handles_small_inputs() {
        assert!(kmeans::<3>(&[], 4, 3).is_empty());
        assert_eq!(kmeans(&[[1.0f32, 2.0]], 8, 3).len(), 1);
    }

    #[test]
    fn nearest_picks_closest() {
        let c = [[0.0f32, 0.0], [10.0, 10.0]];
        assert_eq!(nearest(&c, &[9.0, 8.0]), 1);
        assert_eq!(nearest(&c, &[1.0, 2.0]), 0);
    }
}
