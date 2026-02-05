#!/usr/bin/env python3

"""
Monegle RPC Test Results Analyzer

Analyzes JSON metrics from RPC throughput tests and provides recommendations.
"""

import json
import sys
from pathlib import Path
from statistics import mean, median, stdev
from typing import Dict, List, Any

def analyze_metrics(file_path: Path) -> Dict[str, Any]:
    """Analyze a single test result file."""
    with open(file_path) as f:
        metrics = json.load(f)

    if not metrics:
        return {
            'file': file_path.name,
            'error': 'No metrics found',
        }

    total = len(metrics)
    successful = sum(1 for m in metrics if m['success'])
    failed = total - successful

    latencies = [m['latency_ms'] for m in metrics if m.get('latency_ms') is not None]
    gas_used = [m['gas_used'] for m in metrics if m.get('gas_used') is not None]

    # Calculate percentiles
    sorted_latencies = sorted(latencies)
    p50 = sorted_latencies[len(sorted_latencies) // 2] if sorted_latencies else 0
    p95 = sorted_latencies[int(len(sorted_latencies) * 0.95)] if sorted_latencies else 0
    p99 = sorted_latencies[int(len(sorted_latencies) * 0.99)] if sorted_latencies else 0

    # Detect rate limiting patterns
    errors = [m.get('error', '') for m in metrics if m.get('error')]
    rate_limited = sum(1 for e in errors if '429' in e or 'rate' in e.lower())

    return {
        'file': file_path.name,
        'total_tx': total,
        'successful': successful,
        'failed': failed,
        'success_rate': (successful / total) * 100 if total > 0 else 0,
        'avg_latency': mean(latencies) if latencies else 0,
        'median_latency': median(latencies) if latencies else 0,
        'p95_latency': p95,
        'p99_latency': p99,
        'max_latency': max(latencies) if latencies else 0,
        'avg_gas': mean(gas_used) if gas_used else 0,
        'total_gas': sum(gas_used) if gas_used else 0,
        'total_data_kb': sum(m['data_size'] for m in metrics) / 1024,
        'rate_limited_count': rate_limited,
    }

def print_header():
    """Print header banner."""
    print("\n╔═══════════════════════════════════════════════════════════════════════════════╗")
    print("║              MONEGLE RPC TEST RESULTS ANALYSIS                                ║")
    print("╚═══════════════════════════════════════════════════════════════════════════════╝\n")

def print_comparison_table(results: List[Dict[str, Any]]):
    """Print comparison table of all RPC endpoints."""
    print("╔═══════════════════════════════════════════════════════════════════════════════╗")
    print("║                         RPC ENDPOINT COMPARISON                               ║")
    print("╠═══════════════════════════════════════════════════════════════════════════════╣")
    print("║ Endpoint          Success Rate   Avg Lat   P95 Lat   P99 Lat   Rate Limited  ║")
    print("╠═══════════════════════════════════════════════════════════════════════════════╣")

    for r in results:
        if 'error' in r:
            print(f"║ {r['file']:<17} ERROR: {r['error']:<54} ║")
        else:
            print(f"║ {r['file']:<17} {r['success_rate']:>6.1f}%     "
                  f"{r['avg_latency']:>6.0f}ms  {r['p95_latency']:>6.0f}ms  "
                  f"{r['p99_latency']:>6.0f}ms  {r['rate_limited_count']:>6}       ║")

    print("╚═══════════════════════════════════════════════════════════════════════════════╝\n")

def print_detailed_results(results: List[Dict[str, Any]]):
    """Print detailed results for each endpoint."""
    print("╔═══════════════════════════════════════════════════════════════════════════════╗")
    print("║                           DETAILED RESULTS                                    ║")
    print("╚═══════════════════════════════════════════════════════════════════════════════╝\n")

    for r in results:
        if 'error' in r:
            continue

        print(f"━━━ {r['file']} ━━━")
        print(f"  Transactions:    {r['total_tx']} total, {r['successful']} successful, {r['failed']} failed")
        print(f"  Success Rate:    {r['success_rate']:.1f}%")
        print(f"  Latency:         avg={r['avg_latency']:.0f}ms, median={r['median_latency']:.0f}ms, "
              f"p95={r['p95_latency']:.0f}ms, p99={r['p99_latency']:.0f}ms, max={r['max_latency']:.0f}ms")
        print(f"  Gas:             avg={r['avg_gas']:.0f}, total={r['total_gas']}")
        print(f"  Data:            {r['total_data_kb']:.1f} KB total")
        print(f"  Rate Limited:    {r['rate_limited_count']} transactions")
        print()

def print_recommendations(results: List[Dict[str, Any]]):
    """Print recommendations based on results."""
    print("╔═══════════════════════════════════════════════════════════════════════════════╗")
    print("║                            RECOMMENDATIONS                                    ║")
    print("╠═══════════════════════════════════════════════════════════════════════════════╣")

    # Filter out errors
    valid_results = [r for r in results if 'error' not in r]

    if not valid_results:
        print("║ ❌ No valid test results found                                               ║")
        print("╚═══════════════════════════════════════════════════════════════════════════════╝\n")
        return

    best = max(valid_results, key=lambda r: r['success_rate'])

    print(f"║ Best Endpoint: {best['file']:<59} ║")
    print(f"║   Success Rate: {best['success_rate']:.1f}%{' ' * 57}║")
    print(f"║   Avg Latency:  {best['avg_latency']:.0f}ms{' ' * 58}║")
    print("║                                                                               ║")

    if best['success_rate'] >= 95:
        print("║ ✅ EXCELLENT RESULTS                                                         ║")
        print("║                                                                               ║")
        print("║ This RPC endpoint is highly suitable for production use.                     ║")
        print("║                                                                               ║")
        print("║ Next Steps:                                                                   ║")
        print("║   1. Proceed with full Monegle implementation                                ║")
        print("║   2. Deploy relay and receiver components                                    ║")
        print("║   3. Run end-to-end integration tests                                        ║")

    elif best['success_rate'] >= 80:
        print("║ ⚠️  MODERATE RESULTS                                                          ║")
        print("║                                                                               ║")
        print("║ RPC endpoint works but has reliability issues.                               ║")
        print("║                                                                               ║")
        print("║ Recommended Mitigations:                                                      ║")
        print("║   1. Implement RPC rotation with multiple endpoints                          ║")
        print("║   2. Add transaction retry logic (max 3 retries)                             ║")
        print("║   3. Consider reducing FPS to 10-12 for better reliability                   ║")
        print("║   4. Monitor rate limiting and implement backoff                             ║")

    else:
        print("║ ❌ POOR RESULTS                                                               ║")
        print("║                                                                               ║")
        print("║ RPC endpoint is not suitable for production use in current state.            ║")
        print("║                                                                               ║")
        print("║ Critical Actions Required:                                                    ║")
        print("║   1. Use paid RPC service (Alchemy: $50/mo, Chainstack: $79/mo)             ║")
        print("║   2. Significantly reduce FPS (try 5-8 FPS)                                  ║")
        print("║   3. Test alternative RPC providers                                          ║")
        print("║   4. Consider alternative architectures (IPFS + on-chain pointers)           ║")

    print("║                                                                               ║")

    # Check for high latency
    if any(r['avg_latency'] > 2000 for r in valid_results):
        print("║ ⚠️  HIGH LATENCY DETECTED                                                     ║")
        print("║                                                                               ║")
        print("║ Average latency >2 seconds will cause buffering issues.                      ║")
        print("║ Receivers may experience stuttering or delays.                               ║")
        print("║                                                                               ║")

    # Check for rate limiting
    if any(r['rate_limited_count'] > 0 for r in valid_results):
        print("║ ⚠️  RATE LIMITING DETECTED                                                    ║")
        print("║                                                                               ║")
        print("║ RPC provider is rate limiting requests.                                      ║")
        print("║ Implement exponential backoff and RPC rotation.                              ║")
        print("║                                                                               ║")

    print("╚═══════════════════════════════════════════════════════════════════════════════╝\n")

def main():
    """Main analysis function."""
    results_dir = Path('test-results')

    if not results_dir.exists():
        print("❌ Error: No test-results/ directory found")
        print("\nRun tests first: ./scripts/test-all-rpcs.sh")
        return 1

    json_files = list(results_dir.glob('*.json'))

    if not json_files:
        print("❌ Error: No test result files found in test-results/")
        return 1

    print_header()

    print(f"Found {len(json_files)} test result file(s)\n")

    # Analyze all results
    results = []
    for json_file in json_files:
        try:
            result = analyze_metrics(json_file)
            results.append(result)
        except Exception as e:
            print(f"⚠️  Error analyzing {json_file.name}: {e}")

    if not results:
        print("❌ No valid results to analyze")
        return 1

    # Sort by success rate
    results.sort(key=lambda r: r.get('success_rate', 0), reverse=True)

    # Print results
    print_comparison_table(results)
    print_detailed_results(results)
    print_recommendations(results)

    # Export summary
    summary_path = results_dir / 'analysis-summary.json'
    with open(summary_path, 'w') as f:
        json.dump(results, f, indent=2)

    print(f"✓ Analysis summary saved to: {summary_path}\n")

    return 0

if __name__ == '__main__':
    sys.exit(main())
