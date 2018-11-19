use history::History;
use settings::Settings;

use history::Features;
use node::Node;
use network::Network;
use training_sample_generator::TrainingSampleGenerator;

#[derive(Debug)]
pub struct Trainer<'a> {
    settings: &'a Settings,
    history: &'a mut History,
}

impl<'a> Trainer<'a> {
    pub fn new(settings: &'a Settings, history: &'a mut History) -> Trainer<'a> {
        Trainer { settings, history }
    }

    pub fn train(&mut self) {
        let lr = 0.000002;
        let momentum = 0.5;
        let batch_size = 750;
        let plateau_threshold = 3000;
        let generator = TrainingSampleGenerator::new(self.settings, self.history);

        println!("Evaluating error rate on current {:#?}", self.history.network);
        let mut best_overall_network = self.history.network.clone();
        let mut best_overall_error = self.history.network.average_error(&generator, batch_size * 10);
        println!("Current network error rate is {}", best_overall_error);

        loop {
            let mut best_restart_network = Network::random();
            let mut best_restart_error = 10000.0;
            let mut cycles_since_best_restart_error = 0;
            let mut network = Network::random();
            let mut node_increments = [Node::empty()];
            let mut output_increments = [0.0, 0.0];

            loop {
                let mut batch_error = 0.0;
                let mut batch_samples = 0.0;

                //             b_1
                //                \
                //        f_1 --- s_1 -- o_1
                //            \ /           \
                //             x       b_3 -- s_3 -> o_3 -> e
                //            / \           /
                //        f_2 --- s_2 -- o_2
                //                /
                //             b_2
                //
                // Error (e) = 0.5(t - o_3)^2
                // Final output (o_3) = tanh(s_3)
                // Final sum (s_3) = b_3 + w3_1*o_1 + w3_2*o_2
                // Hidden node 1 output (o_1) = tanh(s_1)
                // Hidden node 1 sum (s_1) = b_1 + w1_1*f_1 + w1_2*f_2
                // Hidden node 2 output (o_2) = tanh(s_2)
                // Hidden node 2 sum (s_2) = b_2 + w2_1*f_1 + w2_2*f_2
                // Derivative of error with respect to o_3 (d_e/d_o_3 0.5(t - o_3)^2): -(t - o_3)
                // Derivative of o_3 respect to s_3 (d_o_3/d_s_3 tanh(s_3)): 1.0 - tanh(s_3)^2
                // Derivative of s_3 with respect to weight w3_1 (d_s_3/d_w3_1 bias + w3_1*o_1 + w3_2*o_2): o_1
                // Derivative of error with respect to weight w3_1 (d_e/d_o_3 * d_o_3/d_s_3 * d_s_3/d_w3_1): -(t - o_3) * (1 - tanh(s_3)^2) * o_1
                // Derivative of s_3 with respect to o_1 (d_s_3/d_o_1 b_3 + w3_1*o_1 + w3_2*o_2): w3_1
                // Derivative of o_1 with respect to s_1 (d_o_1/d_s_1): 1.0 - tanh(s_1)^2
                // Derivative of s_1 with respect to weight w1_1 (d_s_1/d_w1_1): f_1
                // Full derivation of o_3: tanh(b_3 + w3_1*tanh(b_1 + w1_1*f_1 + w1_2*f_2) + w3_2*tanh(b_2 + w2_1*f_1 + w2_2*f_2))
                // Full derivation of s_3: b_3 + w3_1*tanh(b_1 + w1_1*f_1 + w1_2*f_2) + w3_2*tanh(b_2 + w2_1*f_1 + w2_2*f_2)
                // Full error derivation: 0.5(t - tanh(b_3 + w3_1*tanh(b_1 + w1_1*f_1 + w1_2*f_2) + w3_2*tanh(b_2 + w2_1*f_1 + w2_2*f_2)))^2
                // Full derivative for o_1: -(t - o_3) * (1.0 - tanh(s_3)^2) * w3_1
                // Full derivative for w1_1: -(t - o_3) * (1.0 - tanh(s_3)^2) * w3_1 * (1.0 - tanh(s_1)^2) * f_1
                // Checked: https://www.wolframcloud.com/objects/617707c2-5016-4fb3-b73d-bd688b884967
                generator.generate(Some(batch_size), |features: &Features, correct: bool| {
                    let target = if correct { 1.0 } else { -1.0 };
                    network.compute(features);

                    let error = 0.5 * (target - network.final_output).powi(2);
                    batch_error += error;
                    batch_samples += 1.0;

                    let d_e_d_o_3 = -(target - network.final_output);
                    let d_o_3_d_s_3 = 1.0 - network.final_sum.tanh().powi(2);

                    // Output bias
                    output_increments[0] = momentum * output_increments[0] + lr * d_e_d_o_3 * d_o_3_d_s_3 * 1.0;
                    // Final sum node 1 output weight
                    output_increments[1] = momentum * output_increments[1] + lr * d_e_d_o_3 * d_o_3_d_s_3 * network.hidden_node_outputs[0];
                    // Final sum node 2 output weight
                    // output_increments[2] = momentum * output_increments[2] + lr * d_e_d_o_3 * d_o_3_d_s_3 * network.hidden_node_outputs[1];

                    let d_s_3_d_o_1 = network.final_weights[0];
                    let d_o_1_d_s_1 = 1.0 - network.hidden_node_sums[0].tanh().powi(2);
                    let d_e_d_s_1 = d_e_d_o_3 * d_o_3_d_s_3 * d_s_3_d_o_1 * d_o_1_d_s_1;

                    node_increments[0].offset = momentum * node_increments[0].offset + lr * d_e_d_s_1 * 1.0;
                    node_increments[0].age = momentum * node_increments[0].age + lr * d_e_d_s_1 * features.age_factor;
                    node_increments[0].length = momentum * node_increments[0].length + lr * d_e_d_s_1 * features.length_factor;
                    node_increments[0].exit = momentum * node_increments[0].exit + lr * d_e_d_s_1 * features.exit_factor;
                    node_increments[0].recent_failure = momentum * node_increments[0].recent_failure + lr * d_e_d_s_1 * features.recent_failure_factor;
                    node_increments[0].selected_dir = momentum * node_increments[0].selected_dir + lr * d_e_d_s_1 * features.selected_dir_factor;
                    node_increments[0].dir = momentum * node_increments[0].dir + lr * d_e_d_s_1 * features.dir_factor;
                    node_increments[0].overlap = momentum * node_increments[0].overlap + lr * d_e_d_s_1 * features.overlap_factor;
                    node_increments[0].immediate_overlap = momentum * node_increments[0].immediate_overlap + lr * d_e_d_s_1 * features.immediate_overlap_factor;
                    node_increments[0].selected_occurrences = momentum * node_increments[0].selected_occurrences + lr * d_e_d_s_1 * features.selected_occurrences_factor;
                    node_increments[0].occurrences = momentum * node_increments[0].occurrences + lr * d_e_d_s_1 * features.occurrences_factor;

                    let node1 = network.hidden_nodes[0];
                    network = Network {
                        hidden_nodes: [
                            Node {
                                offset: node1.offset - node_increments[0].offset,
                                age: node1.age - node_increments[0].age,
                                length: node1.length - node_increments[0].length,
                                exit: node1.exit - node_increments[0].exit,
                                recent_failure: node1.recent_failure - node_increments[0].recent_failure,
                                selected_dir: node1.selected_dir - node_increments[0].selected_dir,
                                dir: node1.dir - node_increments[0].dir,
                                overlap: node1.overlap - node_increments[0].overlap,
                                immediate_overlap: node1.immediate_overlap - node_increments[0].immediate_overlap,
                                selected_occurrences: node1.selected_occurrences - node_increments[0].selected_occurrences,
                                occurrences: node1.occurrences - node_increments[0].occurrences,
                            }
                        ],
                        hidden_node_sums: [0.0],
                        hidden_node_outputs: [0.0],
                        final_bias: network.final_bias - output_increments[0],
                        final_weights: [network.final_weights[0] - output_increments[1]],
                        final_sum: 0.0,
                        final_output: 0.0,
                    };
                });

                if batch_error / batch_samples < best_restart_error {
                    best_restart_error = batch_error / batch_samples;
                    best_restart_network = network.clone();
                    cycles_since_best_restart_error = 0;
                } else {
                    cycles_since_best_restart_error += 1;
                    if cycles_since_best_restart_error > plateau_threshold {
                        println!("Plateaued at {}.", batch_error / batch_samples);

                        if best_restart_error < best_overall_error {
                            best_overall_error = best_restart_error;
                            best_overall_network = best_restart_network;

                            println!("New best overall for {:#?} with error {} (new best)", best_overall_network, best_overall_error);
                        } else {
                            println!("Best overall remains {:#?} with error {} (old)", best_overall_network, best_overall_error);
                        }
                        break;
                    }
                }

//                println!("Error of {} (vs {} {} ago)", batch_error / batch_samples, best_restart_error, cycles_since_best_restart_error);
            }
        }
    }
}
