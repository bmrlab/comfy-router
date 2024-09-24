use super::{ComfyUIPrompt, ControlNetPayload, CurrentNodeId, Image, LoRAPayload, Model};
use crate::workflow::fetch::FetchHelper;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SD15WorkflowPayload {
    checkpoint: Model,
    vae: Option<Model>,
    loras: Vec<LoRAPayload>,
    controlnets: Vec<ControlNetPayload>,
    prompt: String,
    negative_prompt: String,
    input_image: Option<Image>,
    input_mask: Option<Image>,
    denoise: Option<f32>,
    width: u32,
    height: u32,
    batch_size: u32,
    sampler: String,
    scheduler: String,
    steps: u32,
    cfg_scale: f32,
    seed: Option<u64>,
}

impl SD15WorkflowPayload {
    #[tracing::instrument(skip_all)]
    pub async fn into_comfy_prompt(
        &self,
        fetch_helper: FetchHelper,
    ) -> anyhow::Result<ComfyUIPrompt> {
        let mut fetch_helper = fetch_helper;

        let mut prompt = HashMap::<String, Value>::new();
        let mut current_node_id = CurrentNodeId::new();

        let mut unet_node;
        let mut clip_node;
        let mut vae_node;
        let mut positive_node;
        let mut negative_node;

        // checkpoint
        let load_checkpoint_node_id = current_node_id.get();
        let checkpoint_name = fetch_helper
            .add(&self.checkpoint, "models/checkpoints")
            .await;
        prompt.insert(
            load_checkpoint_node_id.clone(),
            json!({
                "inputs": {"ckpt_name": checkpoint_name},
                "class_type": "CheckpointLoaderSimple",
            }),
        );
        unet_node = (load_checkpoint_node_id.clone(), 0);
        clip_node = (load_checkpoint_node_id.clone(), 1);
        vae_node = (load_checkpoint_node_id.clone(), 2);

        // vae
        if let Some(vae) = &self.vae {
            let load_vae_node_id = current_node_id.get();
            let vae_name = fetch_helper.add(vae, "models/vae").await;

            prompt.insert(
                load_checkpoint_node_id.clone(),
                json!({
                    "inputs": {"vae_name": vae_name},
                    "class_type": "VAELoader",
                }),
            );
            vae_node = (load_vae_node_id.clone(), 0);
        }

        // loras
        for lora in self.loras.iter() {
            let name = fetch_helper.add(&lora.model, "models/loras").await;

            let current_lora_node_id = current_node_id.get();
            prompt.insert(
                current_lora_node_id.clone(),
                json!({
                    "inputs": {
                        "lora_name": name,
                        "strength_model": lora.weight,
                        "strength_clip": lora.weight,
                        "model": [unet_node.0, unet_node.1],
                        "clip": [clip_node.0, clip_node.1],
                    },
                    "class_type": "LoraLoader"
                }),
            );
            unet_node = (current_lora_node_id.clone(), 0);
            clip_node = (current_lora_node_id.clone(), 1);
        }

        // prompt and negative prompt
        positive_node = (current_node_id.get(), 0);
        negative_node = (current_node_id.get(), 0);

        prompt.insert(
            positive_node.0.clone(),
            json!({
                "inputs": {
                            "text": self.prompt,
                            "clip": [clip_node.0, clip_node.1]
                        },
                "class_type": "CLIPTextEncode",
            }),
        );
        prompt.insert(
            negative_node.0.clone(),
            json!({
                "inputs": {
                            "text": self.negative_prompt,
                            "clip": [clip_node.0, clip_node.1]
                        },
                "class_type": "CLIPTextEncode",
            }),
        );

        // controlnets
        for controlnet in self.controlnets.iter() {
            let load_controlnet_node_id = current_node_id.get();
            let name = fetch_helper
                .add(&controlnet.model, "models/controlnet")
                .await;

            prompt.insert(
                load_controlnet_node_id.clone(),
                json!({
                    "inputs": {
                        "control_net_name": name,
                    },
                    "class_type": "ControlNetLoader"
                }),
            );

            // preprocessor
            // - load image, resize, pass through preprocessor
            let load_image_node_id = current_node_id.get();
            let resize_node_id = current_node_id.get();
            let mut preprocessor_node_id = current_node_id.get();

            let image_name = fetch_helper.add(&controlnet.image, "input").await;
            prompt.insert(
                load_image_node_id.clone(),
                json!({
                    "inputs": {
                        "image": image_name,
                    },
                    "class_type": "LoadImage"
                }),
            );
            prompt.insert(
                resize_node_id.clone(),
                json!({
                    "inputs": {
                        "hint_image": [load_image_node_id, 0],
                        "image_gen_width": self.width,
                        "image_gen_height": self.height,
                        "resize_mode": controlnet.resize_mode
                    },
                    "class_type": "HintImageEnchance"
                }),
            );

            if let Some(class_type) = &controlnet.preprocessor {
                let mut inputs = controlnet.preprocessor_params.clone().unwrap_or(json!({}));
                inputs["image"] = json!([resize_node_id, 0]);
                prompt.insert(
                    preprocessor_node_id.clone(),
                    json!({
                        "inputs": inputs,
                        "class_type": class_type
                    }),
                );
            } else {
                preprocessor_node_id = resize_node_id.clone()
            }

            let apply_controlnet_node_id = current_node_id.get();

            prompt.insert(
                apply_controlnet_node_id.clone(),
                json!({
                    "inputs": {
                        "positive": [positive_node.0, positive_node.1],
                        "negative": [negative_node.0, negative_node.1],
                        "control_net": [load_controlnet_node_id, 0],
                        "image": [preprocessor_node_id, 0],
                        "strength": controlnet.weight,
                        "start_percent": controlnet.start_at,
                        "end_percent": controlnet.end_at,
                    },
                    "class_type": "ControlNetApplyAdvanced"
                }),
            );

            positive_node = (apply_controlnet_node_id.clone(), 0);
            negative_node = (apply_controlnet_node_id.clone(), 1);
        }

        // latent image
        let (latent_image_node, denoise) = match &self.input_image {
            Some(image) => {
                let load_image_node_id = current_node_id.get();
                let image_name = fetch_helper.add(image, "input").await;
                prompt.insert(
                    load_image_node_id.clone(),
                    json!({
                        "inputs": {
                            "image": image_name,
                        },
                        "class_type": "LoadImage"
                    }),
                );

                let resized_image_node_id = current_node_id.get();
                prompt.insert(
                    resized_image_node_id.clone(),
                    json!({
                        "inputs": {
                            "image": [load_image_node_id, 0],
                            "image_gen_width": self.width,
                            "image_gen_height": self.height,
                            "resize_mode": "Crop and Resize"
                        },
                        "class_type": "HintImageEnchance"
                    }),
                );

                if let Some(mask) = &self.input_mask {
                    let load_mask_node_id = current_node_id.get();
                    let mask_name = fetch_helper.add(mask, "input").await;
                    prompt.insert(
                        load_mask_node_id.clone(),
                        json!({
                            "inputs": {
                                "image": mask_name,
                                "channel": "red"
                            },
                            "class_type": "LoadImageMask"
                        }),
                    );

                    let resized_mask_node_id = current_node_id.get();
                    prompt.insert(
                        resized_mask_node_id.clone(),
                        json!({
                            "inputs": {
                                "image": [load_mask_node_id, 0],
                                "image_gen_width": self.width,
                                "image_gen_height": self.height,
                                "resize_mode": "Crop and Resize"
                            },
                            "class_type": "HintImageEnchance"
                        }),
                    );

                    let vae_encode_node_id = current_node_id.get();
                    prompt.insert(
                        vae_encode_node_id.clone(),
                        json!({
                            "inputs": {
                                "pixels": [resized_image_node_id, 0],
                                "vae": [vae_node.0, vae_node.1],
                                "mask": [resized_mask_node_id, 0],
                                "grow_mask_by": 6
                            },
                            "class_type": "VAEEncodeForInpaint"
                        }),
                    );

                    ((vae_encode_node_id, 0), 1.0)
                } else {
                    // just vae encode
                    let vae_encode_node_id = current_node_id.get();
                    prompt.insert(
                        vae_encode_node_id.clone(),
                        json!({
                            "inputs": {
                                "pixels": [resized_image_node_id, 0],
                                "vae": [vae_node.0, vae_node.1],
                            },
                            "class_type": "VAEEncodeForInpaint"
                        }),
                    );
                    ((vae_encode_node_id, 0), self.denoise.unwrap_or(0.75))
                }
            }
            _ => {
                // empty latent image
                let node_id = current_node_id.get();

                prompt.insert(
                    node_id.clone(),
                    json!({
                        "inputs": {
                            "width": self.width,
                            "height": self.height,
                            "batch_size": self.batch_size,
                        },
                        "class_type": "EmptyLatentImage"
                    }),
                );

                ((node_id, 0), 1.0)
            }
        };

        // KSampler
        let k_sampler_node_id = current_node_id.get();
        prompt.insert(
            k_sampler_node_id.clone(),
            json!({
                "inputs": {
                    "seed": self.seed.unwrap_or(0),
                    "steps": self.steps,
                    "cfg": self.cfg_scale,
                    "sampler_name": self.sampler,
                    "scheduler": self.scheduler,
                    "denoise": denoise,
                    "model": [
                        unet_node.0,
                        unet_node.1
                    ],
                    "positive": [
                        positive_node.0,
                        positive_node.1
                    ],
                    "negative": [
                        negative_node.0,
                        negative_node.1
                    ],
                    "latent_image": [
                        latent_image_node.0,
                        latent_image_node.1
                    ]
                },
                "class_type": "KSampler",
            }),
        );

        // vae decode
        let vae_decode_node_id = current_node_id.get();
        prompt.insert(
            vae_decode_node_id.clone(),
            json!({
                "inputs": {
                    "samples": [
                        k_sampler_node_id, 0
                    ],
                    "vae": [vae_node.0, vae_node.1]
                },
                "class_type": "VAEDecode"
            }),
        );

        // output
        let output_node_id = current_node_id.get();
        prompt.insert(
            output_node_id.clone(),
            json!({
                "inputs": {
                    "images": [vae_decode_node_id, 0]
                },
                "class_type": "SaveImageWebsocket"
            }),
        );

        tracing::debug!("comfyui prompt: {:?}", json!(prompt).to_string());

        fetch_helper.wait_all().await?;

        Ok(ComfyUIPrompt {
            prompt: json!(prompt),
            k_sampler_node_id: k_sampler_node_id.clone(),
            output_node_id: output_node_id.clone(),
        })
    }
}
