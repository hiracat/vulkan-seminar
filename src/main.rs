use std::{ffi, ptr};

use ash::vk::{self, RenderingAttachmentInfo, SurfaceKHR};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event_loop::{ActiveEventLoop, EventLoop},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window, WindowAttributes},
};
const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 600;
const FRAMES_IN_FLIGHT: u32 = 3;

fn main() {
    let mut app = App::new();
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut app).unwrap();
}

struct App {
    vulkan_state: Option<VulkanState>,
}
impl App {
    fn new() -> App {
        App { vulkan_state: None }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.vulkan_state.is_none() {
            self.vulkan_state = Some(init(event_loop))
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::RedrawRequested => {
                let state = self.vulkan_state.as_mut().unwrap();
                let frame = &mut state.per_frame[state.frame_in_flight];
                unsafe {
                    state
                        .device
                        .wait_for_fences(&[frame.in_flight], false, u64::MAX)
                        .unwrap();
                }
                // ignore suboptiomal for now, makes code more complicated than necessary
                let (image_index, _suboptimal) = unsafe {
                    state
                        .swapchain_loader
                        .acquire_next_image(
                            state.swapchain,
                            u64::MAX,
                            frame.image_available,
                            vk::Fence::null(),
                        )
                        .unwrap()
                };
                state.swapchain_image_index = image_index as usize;
                unsafe { state.device.reset_fences(&[frame.in_flight]).unwrap() };
                unsafe {
                    state
                        .device
                        .reset_command_pool(frame.command_pool, vk::CommandPoolResetFlags::empty())
                        .unwrap();
                }
                unsafe {
                    state
                        .device
                        .begin_command_buffer(
                            frame.command_buffer,
                            &vk::CommandBufferBeginInfo {
                                flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                                ..Default::default()
                            },
                        )
                        .unwrap()
                };
                let mut swapchain_barrier = vk::ImageMemoryBarrier2 {
                    image: state.swapchain_images[state.swapchain_image_index],
                    old_layout: vk::ImageLayout::UNDEFINED,
                    new_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    src_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                    src_access_mask: vk::AccessFlags2::empty(),
                    dst_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                    dst_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        level_count: 1,
                        layer_count: 1,
                        base_mip_level: 0,
                        base_array_layer: 0,
                    },
                    ..Default::default()
                };
                unsafe {
                    state.device.cmd_pipeline_barrier2(
                        frame.command_buffer,
                        &vk::DependencyInfo {
                            image_memory_barrier_count: 1,
                            p_image_memory_barriers: &mut swapchain_barrier as *mut _,
                            ..Default::default()
                        },
                    );
                };
                let window_size = state.image_size;
                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: window_size,
                };
                let viewport = vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: window_size.width as f32,
                    height: window_size.height as f32,
                    max_depth: 1.0,
                    min_depth: 0.0,
                };
                unsafe {
                    state.device.cmd_begin_rendering(
                        frame.command_buffer,
                        &vk::RenderingInfo {
                            render_area: vk::Rect2D {
                                offset: vk::Offset2D { x: 0, y: 0 },
                                extent: window_size,
                            },
                            layer_count: 1,
                            color_attachment_count: 1,
                            p_color_attachments: &RenderingAttachmentInfo {
                                image_view: state.swapchain_image_views
                                    [state.swapchain_image_index],
                                image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                                load_op: vk::AttachmentLoadOp::CLEAR,
                                store_op: vk::AttachmentStoreOp::STORE,
                                clear_value: vk::ClearValue {
                                    color: vk::ClearColorValue {
                                        float32: [0.0, 0.0, 0.0, 0.0],
                                    },
                                },
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    );
                    state.device.cmd_bind_pipeline(
                        frame.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        state.pipeline,
                    );
                    state
                        .device
                        .cmd_set_viewport(frame.command_buffer, 0, &[viewport]);
                    state
                        .device
                        .cmd_set_scissor(frame.command_buffer, 0, &[scissor]);
                    state.device.cmd_draw(frame.command_buffer, 3, 1, 0, 0);
                    state.device.cmd_end_rendering(frame.command_buffer);
                }
                // begin presentation

                let present_barrier = vk::ImageMemoryBarrier2 {
                    image: state.swapchain_images[state.swapchain_image_index],
                    old_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    new_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                    src_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                    src_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
                    dst_stage_mask: vk::PipelineStageFlags2::empty(),
                    dst_access_mask: vk::AccessFlags2::empty(),
                    src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                    dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        level_count: 1,
                        layer_count: 1,
                        base_mip_level: 0,
                        base_array_layer: 0,
                    },
                    ..Default::default()
                };
                let barriers = [present_barrier];
                unsafe {
                    state.device.cmd_pipeline_barrier2(
                        frame.command_buffer,
                        &vk::DependencyInfo::default().image_memory_barriers(&barriers),
                    );
                    state
                        .device
                        .end_command_buffer(frame.command_buffer)
                        .unwrap();
                }

                // submit
                let wait_info = [vk::SemaphoreSubmitInfo::default()
                    .semaphore(frame.image_available)
                    .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)];
                let cmd_info =
                    [vk::CommandBufferSubmitInfo::default().command_buffer(frame.command_buffer)];
                let signal_info = [vk::SemaphoreSubmitInfo::default()
                    .semaphore(state.render_finished[state.swapchain_image_index])
                    .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)];
                let submit = vk::SubmitInfo2::default()
                    .wait_semaphore_infos(&wait_info)
                    .command_buffer_infos(&cmd_info)
                    .signal_semaphore_infos(&signal_info);
                unsafe {
                    state
                        .device
                        .queue_submit2(state.queue, &[submit], frame.in_flight)
                        .unwrap();
                }

                // present
                let swapchains = [state.swapchain];
                let indices = [image_index];
                let wait = [state.render_finished[state.swapchain_image_index]];
                let present = vk::PresentInfoKHR::default()
                    .wait_semaphores(&wait)
                    .swapchains(&swapchains)
                    .image_indices(&indices);
                unsafe {
                    state
                        .swapchain_loader
                        .queue_present(state.queue, &present)
                        .unwrap();
                }

                state.frame_in_flight = (state.frame_in_flight + 1) % state.per_frame.len();
                state.window.request_redraw();
            }
            winit::event::WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => (),
        }
    }
}

struct VulkanState {
    _entry: ash::Entry,
    _instance: ash::Instance,
    _surface_loader: ash::khr::surface::Instance,
    _surface: SurfaceKHR,
    _physical_device: vk::PhysicalDevice,

    window: Window,
    queue: vk::Queue,
    device: ash::Device,

    swapchain: vk::SwapchainKHR,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    render_finished: Vec<vk::Semaphore>,

    image_size: vk::Extent2D,
    frame_in_flight: usize,
    per_frame: Vec<PerFrame>,
    swapchain_image_index: usize,

    pipeline: vk::Pipeline,
}

fn init(event_loop: &ActiveEventLoop) -> VulkanState {
    let entry = unsafe { ash::Entry::load().unwrap() };
    let window = create_window(event_loop);
    let instance = create_instance(&entry, event_loop);
    // this just loads the correct function pointers from the library
    // its basically an extension of the regular instance with extra pointers
    let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
    let surface = unsafe {
        ash_window::create_surface(
            &entry,
            &instance,
            // idk what to do about this, i need the raw handle
            event_loop.display_handle().unwrap().as_raw(),
            window.window_handle().unwrap().as_raw(),
            None,
        )
        .unwrap()
    };

    let required_extensions = [ash::vk::KHR_SWAPCHAIN_NAME];
    let (physical_device, queue_family_index) =
        create_physical_device(&instance, surface, &required_extensions);
    let (mut device, queue) = create_device(
        &instance,
        physical_device,
        queue_family_index,
        &required_extensions,
    );

    let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
    let capabilities = unsafe {
        surface_loader.get_physical_device_surface_capabilities(physical_device, surface)
    }
    .unwrap();
    let window_size = if capabilities.current_extent.width != u32::MAX {
        capabilities.current_extent
    } else {
        let inner = window.inner_size();
        vk::Extent2D {
            width: inner.width.clamp(
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
            ),
            height: inner.height.clamp(
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
            ),
        }
    };

    let image_count = if capabilities.min_image_count == capabilities.max_image_count {
        capabilities.max_image_count
    } else {
        capabilities.min_image_count + 1
    };
    let color_subresource_range = vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        level_count: 1,
        layer_count: 1,
        base_mip_level: 0,
        base_array_layer: 0,
    };
    let swapchain_image_formats = unsafe {
        surface_loader
            .get_physical_device_surface_formats(physical_device, surface)
            .unwrap()
    };
    let swapchain_image_format = choose_swapchain_format(&swapchain_image_formats).unwrap();

    let swapchain_create_info = vk::SwapchainCreateInfoKHR {
        image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        surface,
        min_image_count: image_count,
        image_sharing_mode: vk::SharingMode::EXCLUSIVE,
        image_color_space: swapchain_image_format.color_space,
        image_format: swapchain_image_format.format,
        image_extent: window_size,
        present_mode: vk::PresentModeKHR::FIFO,
        pre_transform: vk::SurfaceTransformFlagsKHR::IDENTITY,
        old_swapchain: vk::SwapchainKHR::null(),
        composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
        clipped: vk::TRUE,
        image_array_layers: 1,
        p_queue_family_indices: &queue_family_index,
        queue_family_index_count: 1,

        ..Default::default()
    };

    let swapchain = unsafe {
        swapchain_loader
            .create_swapchain(&swapchain_create_info, None)
            .unwrap()
    };
    let swapchain_images = unsafe {
        swapchain_loader
            .get_swapchain_images(swapchain)
            .expect("Failed to get Swapchain Images.")
    };

    let mut swapchain_image_views = Vec::new();
    for image in &swapchain_images {
        let swapchain_image_view = unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo {
                    format: swapchain_image_format.format,
                    image: *image,
                    view_type: vk::ImageViewType::TYPE_2D,
                    subresource_range: color_subresource_range,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        swapchain_image_views.push(swapchain_image_view);
    }

    let mut per_frame = Vec::new();
    for _ in 0..FRAMES_IN_FLIGHT {
        per_frame.push(PerFrame::create(queue_family_index, &mut device));
    }

    let render_finished = create_semaphores(&device, swapchain_images.len());

    let vertex_code = r#"
        #version 450
        layout(location = 0) out vec3 fragColor;

        vec2 positions[3] = vec2[](
            vec2( 0.0, -0.5),
            vec2( 0.5,  0.5),
            vec2(-0.5,  0.5)
        );
        vec3 colors[3] = vec3[](
            vec3(1.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            vec3(0.0, 0.0, 1.0)
        );

        void main() {
            gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
            fragColor = colors[gl_VertexIndex];
        }
    "#;
    let fragment_code = r#"
        #version 450
        layout(location = 0) in vec3 fragColor;
        layout(location = 0) out vec4 outColor;

        void main() {
            outColor = vec4(fragColor, 1.0);
        }

    "#;
    let vertex_bin = compile_glsl(vertex_code, shaderc::ShaderKind::Vertex, "vertex");
    let fragment_bin = compile_glsl(fragment_code, shaderc::ShaderKind::Fragment, "fragment");

    let vertex_shader = unsafe {
        device
            .create_shader_module(
                &vk::ShaderModuleCreateInfo {
                    code_size: vertex_bin.len() * 4,
                    p_code: vertex_bin.as_ptr(),
                    ..Default::default()
                },
                None,
            )
            .unwrap()
    };
    let fragment_shader = unsafe {
        device
            .create_shader_module(
                &vk::ShaderModuleCreateInfo {
                    code_size: fragment_bin.len() * 4,
                    p_code: fragment_bin.as_ptr(),
                    ..Default::default()
                },
                None,
            )
            .unwrap()
    };

    // Empty layout: no descriptors, no push constants
    let pipeline_layout = unsafe {
        device
            .create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default(), None)
            .unwrap()
    };

    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_shader)
            .name(c"main"),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_shader)
            .name(c"main"),
    ];

    // No vertex buffers: positions are hardcoded in the shader
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();

    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    // Counts only; the actual viewport/scissor are dynamic
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);

    let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::CLOCKWISE)
        .line_width(1.0);

    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);

    let blend_attachments = [vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(false)];
    let color_blend =
        vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachments);

    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    // Dynamic rendering: attachment formats go here instead of a render pass
    let color_formats = [swapchain_image_format.format]; // the vk::Format you picked for the swapchain
    let mut rendering_info =
        vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&color_formats);

    let create_info = vk::GraphicsPipelineCreateInfo::default()
        .push_next(&mut rendering_info)
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterizer)
        .multisample_state(&multisample)
        .color_blend_state(&color_blend)
        .dynamic_state(&dynamic_state)
        .layout(pipeline_layout);

    let pipeline = unsafe {
        device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[create_info], None)
            .unwrap()[0]
    };

    VulkanState {
        window,
        _surface_loader: surface_loader,
        swapchain_loader,
        _entry: entry,
        _instance: instance,
        device,
        _physical_device: physical_device,
        queue,
        _surface: surface,
        per_frame,
        swapchain,
        swapchain_images,
        swapchain_image_views,
        image_size: window_size,
        render_finished,
        frame_in_flight: 0,
        swapchain_image_index: 0,
        pipeline,
    }
}

fn compile_glsl(src: &str, kind: shaderc::ShaderKind, name: &str) -> Vec<u32> {
    let compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_3 as u32,
    );

    let artifact = compiler
        .compile_into_spirv(src, kind, name, "main", Some(&options))
        .unwrap_or_else(|e| panic!("shader compile error:\n{e}"));

    artifact.as_binary().to_vec()
}

pub fn create_semaphores(device: &ash::Device, count: usize) -> Vec<vk::Semaphore> {
    let mut semaphores = Vec::with_capacity(count);
    for _ in 0..count {
        semaphores.push(unsafe {
            device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                .unwrap()
        });
    }
    semaphores
}

struct PerFrame {
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    in_flight: vk::Fence,
    image_available: vk::Semaphore,
}
impl PerFrame {
    fn create(queue_family_index: u32, device: &mut ash::Device) -> PerFrame {
        let command_pool_create_info = vk::CommandPoolCreateInfo {
            queue_family_index,
            ..Default::default()
        };
        let command_pool = unsafe {
            device
                .create_command_pool(&command_pool_create_info, None)
                .unwrap()
        };
        let command_buffer_alloc_info = vk::CommandBufferAllocateInfo {
            command_pool,
            command_buffer_count: 1,
            level: vk::CommandBufferLevel::PRIMARY,
            ..Default::default()
        };
        let command_buffer = unsafe {
            device
                .allocate_command_buffers(&command_buffer_alloc_info)
                .unwrap()
                .first()
                .copied()
                .unwrap()
        };
        let in_flight = unsafe {
            device.create_fence(
                &vk::FenceCreateInfo {
                    flags: vk::FenceCreateFlags::SIGNALED,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        let image_available = unsafe {
            device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                .unwrap()
        };
        Self {
            command_pool,
            command_buffer,
            in_flight,
            image_available,
        }
    }
}

pub fn create_window(event_loop: &ActiveEventLoop) -> Window {
    event_loop
        .create_window(
            WindowAttributes::default()
                .with_title("Thanks for Coming to my Seminar")
                .with_resizable(false)
                .with_inner_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT)),
        )
        .expect("failed to create window")
}
pub fn create_instance(entry: &ash::Entry, event_loop: &ActiveEventLoop) -> ash::Instance {
    let engine_name = c"hello triangle";
    let app_info = vk::ApplicationInfo {
        api_version: vk::make_api_version(0, 1, 3, 0),
        p_engine_name: engine_name.as_ptr(),
        ..Default::default()
    };

    #[allow(deprecated)]
    let required_instance_extensions =
        ash_window::enumerate_required_extensions(event_loop.display_handle().unwrap().as_raw())
            .unwrap();

    let instance_extensions: Vec<_> = required_instance_extensions.to_vec();

    let create_info = vk::InstanceCreateInfo {
        p_application_info: &app_info,

        enabled_layer_count: 0,
        pp_enabled_layer_names: ptr::null(),

        pp_enabled_extension_names: instance_extensions.as_ptr(),
        enabled_extension_count: instance_extensions.len() as u32,
        ..Default::default()
    };

    unsafe { entry.create_instance(&create_info, None).unwrap() }
}

pub fn create_physical_device(
    instance: &ash::Instance,
    // TODO: should use this to test surface support but dont feel like it
    _surface: vk::SurfaceKHR,
    required_extensions: &[&ffi::CStr],
) -> (vk::PhysicalDevice, u32) {
    let physical_devices = unsafe { instance.enumerate_physical_devices().unwrap() };

    physical_devices
        .into_iter()
        // this is desctructuring, think of it as &physical_device is like a (&,physcal_device) tuple,
        // and its doing let (&,physical_device) = physical_device_reference
        // the reference matches the reference, and the name matches the underlying data
        .filter(|&physical_device| {
            let properties = unsafe {
                instance
                    .enumerate_device_extension_properties(physical_device)
                    .unwrap()
            };

            required_extensions.iter().all(|&required_name| {
                properties.iter().any(|extension_properties| {
                    *required_name == *extension_properties.extension_name_as_c_str().unwrap()
                })
            })
        })
        .filter_map(|physical_device| {
            let queue_family_properties = unsafe {
                let len =
                    instance.get_physical_device_queue_family_properties2_len(physical_device);
                let mut out = vec![vk::QueueFamilyProperties2::default(); len];

                instance.get_physical_device_queue_family_properties2(physical_device, &mut out);
                out
            };

            queue_family_properties
                .iter() // iterate over all available queues for each device
                .enumerate() // returns iterator of (index, queue_family_properties)
                .position(|(_, queue_family_properties)| {
                    queue_family_properties
                        .queue_family_properties
                        .queue_flags
                        .intersects(vk::QueueFlags::GRAPHICS)
                })
                .map(|index| (physical_device, index as u32))
        })
        .min_by_key(|(physical_device, _)| {
            let mut properties = vk::PhysicalDeviceProperties2::default();
            unsafe {
                instance.get_physical_device_properties2(*physical_device, &mut properties);
            };
            match properties.properties.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 0,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
                vk::PhysicalDeviceType::CPU => 3,
                vk::PhysicalDeviceType::OTHER => 4,
                _ => 5,
            }
        })
        .expect("no qualified gpu")
}
pub fn create_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    required_extensions: &[&ffi::CStr],
) -> (ash::Device, vk::Queue) {
    let enabled_features = unsafe { instance.get_physical_device_features(physical_device) };
    let queue_create_infos = [vk::DeviceQueueCreateInfo {
        queue_family_index,
        queue_count: 1,
        p_queue_priorities: [1.0].as_ptr(),
        ..Default::default()
    }];
    let required_extensions: Vec<*const i8> = required_extensions
        .iter()
        .map(|cstr| cstr.as_ptr())
        .collect();

    // 1. lowest level feature
    let mut sync2_features = vk::PhysicalDeviceSynchronization2Features {
        synchronization2: vk::TRUE,
        p_next: std::ptr::null_mut(),
        ..Default::default()
    };

    // 2. dynamic rendering
    let mut dynamic_rendering_features = vk::PhysicalDeviceDynamicRenderingFeatures {
        dynamic_rendering: vk::TRUE,
        p_next: &mut sync2_features as *mut _ as *mut ffi::c_void,
        ..Default::default()
    };

    // 3. demote to helper invocation
    let mut demote_features = vk::PhysicalDeviceShaderDemoteToHelperInvocationFeatures {
        shader_demote_to_helper_invocation: vk::TRUE,
        p_next: &mut dynamic_rendering_features as *mut _ as *mut ffi::c_void,
        ..Default::default()
    };

    let create_info = vk::DeviceCreateInfo {
        p_enabled_features: &enabled_features,
        pp_enabled_extension_names: required_extensions.as_ptr(),
        enabled_extension_count: required_extensions.len() as u32,
        p_queue_create_infos: queue_create_infos.as_ptr(),
        queue_create_info_count: queue_create_infos.len() as u32,
        p_next: &mut demote_features as *mut _ as *mut std::ffi::c_void,

        ..Default::default()
    };

    let device = unsafe {
        instance
            .create_device(physical_device, &create_info, None)
            .unwrap()
    };
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

    (device, queue)
}
pub fn choose_swapchain_format(formats: &[vk::SurfaceFormatKHR]) -> Option<vk::SurfaceFormatKHR> {
    let preferred_color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR;
    let preferred_formats = vec![
        vk::Format::B8G8R8A8_SRGB,
        vk::Format::R8G8B8A8_SRGB,
        vk::Format::B8G8R8A8_UNORM,
        vk::Format::R8G8B8A8_UNORM,
    ];

    for available in formats {
        for format in &preferred_formats {
            if available.format == *format && available.color_space == preferred_color_space {
                return Some(*available);
            }
        }
    }
    formats.first().copied()
}
