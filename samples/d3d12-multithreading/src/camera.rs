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

use cgmath::{
    point3, vec3, EuclideanSpace, InnerSpace, Matrix, Matrix3, Matrix4, Point3, Rad, Transform,
    Vector3,
};

pub struct Camera {
    pub eye: Point3<f32>,
    pub at: Point3<f32>,
    pub up: Vector3<f32>,
}

pub struct ViewAndProjectionMatrices {
    pub view: Matrix4<f32>,
    pub projection: Matrix4<f32>,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            eye: point3(0.0, 15.0, -30.0),
            at: point3(0.0, 8.0, 0.0),
            up: vec3(0.0, 1.0, 0.0),
        }
    }
}

impl Camera {
    pub fn get_3dview_proj_matrices<A>(
        &self,
        fov: A,
        screen_width: f32,
        screen_height: f32,
    ) -> ViewAndProjectionMatrices
    where
        A: Into<Rad<f32>>,
    {
        let aspect_ratio = screen_width / screen_height;
        let fov: Rad<f32> = fov.into();
        let fov_angle_y = if aspect_ratio < 1.0 {
            fov / aspect_ratio
        } else {
            fov
        };

        let projection = cgmath::perspective(fov_angle_y, aspect_ratio, 0.01, 125.0).transpose();
        let view = cgmath::Matrix4::look_at_rh(self.eye, self.at, self.up).transpose();

        ViewAndProjectionMatrices { view, projection }
    }

    pub fn rotate_yaw<A: Into<Rad<f32>>>(&mut self, angle: A) {
        let rotation = Matrix3::from_axis_angle(self.up, angle);
        self.eye = rotation.transform_point(self.eye);
    }

    pub fn rotate_pitch<A: Into<Rad<f32>>>(&mut self, angle: A) {
        let right = self.eye.to_vec().cross(self.up).normalize();
        let rotation = Matrix3::from_axis_angle(right, angle);
        self.eye = rotation.transform_point(self.eye);
    }
}
