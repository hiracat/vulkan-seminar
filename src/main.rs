use std::{ffi, ptr};

use ash::vk::{self, SurfaceKHR};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event_loop::{ActiveEventLoop, EventLoop},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window, WindowAttributes},
};
static WINDOW_WIDTH: u32 = 800;
static WINDOW_HEIGHT: u32 = 600;
static FRAMES_IN_FLIGHT: u32 = 3;

fn main() {
    let mut app = App::new();
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut app).unwrap();
}

struct App {
    window: Option<Window>,
    vulkan_state: Option<VulkanState>,
}
impl App {
    fn new() -> App {
        App {
            window: None,
            vulkan_state: None,
        }
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
            winit::event::WindowEvent::RedrawRequested => {}
            winit::event::WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => (),
        }
    }
}

struct VulkanState {
    entry: ash::Entry,
    instance: ash::Instance,
    window: Window,
    surface_loader: ash::khr::surface::Instance,
    surface: SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    queue: vk::Queue,
    device: ash::Device,

    swapchain: vk::SwapchainKHR,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain_image_format: vk::SurfaceFormatKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    render_finished: Vec<vk::Semaphore>,

    image_size: vk::Extent2D,
}

pub fn init(event_loop: &ActiveEventLoop) -> VulkanState {
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

    VulkanState {
        window,
        surface_loader,
        swapchain_loader,
        entry,
        instance,
        device,
        physical_device,
        queue,
        queue_family_index,
        surface,
        swapchain,
        swapchain_image_format,
        swapchain_images,
        swapchain_image_views,
        image_size: window_size,
        render_finished,
    }
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
            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
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
