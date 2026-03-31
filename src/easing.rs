use num_traits::Float;
use std::f64::consts::{FRAC_PI_2, PI};

fn lit<F: Float>(f: f64) -> F {
    F::from(f).unwrap()
}

#[inline]
pub fn linear<F: Float>(t: F) -> F {
    t
}

#[inline]
pub fn quad_in<F: Float>(t: F) -> F {
    t * t
}

#[inline]
pub fn quad_out<F: Float>(t: F) -> F {
    -t * (t - lit::<F>(2.0))
}

#[inline]
pub fn quad_inout<F: Float>(t: F) -> F {
    if t < lit::<F>(0.5) {
        lit::<F>(2.0) * t * t
    } else {
        (lit::<F>(-2.0) * t * t) + (lit::<F>(4.0) * t) - lit::<F>(1.0)
    }
}

#[inline]
pub fn cubic_in<F: Float>(t: F) -> F {
    t * t * t
}

#[inline]
pub fn cubic_out<F: Float>(t: F) -> F {
    let f = t - lit::<F>(1.0);
    f * f * f + lit::<F>(1.0)
}

#[inline]
pub fn cubic_inout<F: Float>(t: F) -> F {
    if t < lit::<F>(0.5) {
        lit::<F>(4.0) * t * t * t
    } else {
        let f = (lit::<F>(2.0) * t) - lit::<F>(2.0);
        lit::<F>(0.5) * f * f * f + lit::<F>(1.0)
    }
}

#[inline]
pub fn quart_in<F: Float>(t: F) -> F {
    t * t * t * t
}

#[inline]
pub fn quart_out<F: Float>(t: F) -> F {
    let f = t - lit::<F>(1.0);
    f * f * f * (lit::<F>(1.0) - t) + lit::<F>(1.0)
}

#[inline]
pub fn quart_inout<F: Float>(t: F) -> F {
    if t < lit::<F>(0.5) {
        lit::<F>(8.0) * t * t * t * t
    } else {
        let f = t - lit::<F>(1.0);
        lit::<F>(-8.0) * f * f * f * f + lit::<F>(1.0)
    }
}

#[inline]
pub fn quint_in<F: Float>(t: F) -> F {
    t * t * t * t * t
}

#[inline]
pub fn quint_out<F: Float>(t: F) -> F {
    let f = t - lit::<F>(1.0);
    f * f * f * f * f + lit::<F>(1.0)
}

#[inline]
pub fn quint_inout<F: Float>(t: F) -> F {
    if t < lit::<F>(0.5) {
        lit::<F>(16.0) * t * t * t * t * t
    } else {
        let f = (lit::<F>(2.0) * t) - lit::<F>(2.0);
        lit::<F>(0.5) * f * f * f * f * f + lit::<F>(1.0)
    }
}

#[inline]
pub fn sine_in<F: Float>(t: F) -> F {
    ((t - lit::<F>(1.0)) * lit::<F>(FRAC_PI_2)).sin() + lit::<F>(1.0)
}

#[inline]
pub fn sine_out<F: Float>(t: F) -> F {
    (t * lit::<F>(FRAC_PI_2)).sin()
}

#[inline]
pub fn sine_inout<F: Float>(t: F) -> F {
    lit::<F>(0.5) * (lit::<F>(1.0) - (t * lit::<F>(PI)).cos())
}

#[inline]
pub fn circ_in<F: Float>(t: F) -> F {
    lit::<F>(1.0) - (lit::<F>(1.0) - t * t).sqrt()
}

#[inline]
pub fn circ_out<F: Float>(t: F) -> F {
    ((lit::<F>(2.0) - t) * t).sqrt()
}

#[inline]
pub fn circ_inout<F: Float>(t: F) -> F {
    if t < lit::<F>(0.5) {
        lit::<F>(0.5) * (lit::<F>(1.0) - (lit::<F>(1.0) - lit::<F>(4.0) * t * t).sqrt())
    } else {
        lit::<F>(0.5)
            * ((-(lit::<F>(2.0) * t - lit::<F>(3.0)) * (lit::<F>(2.0) * t - lit::<F>(1.0))).sqrt()
                + lit::<F>(1.0))
    }
}

#[inline]
pub fn expo_in<F: Float>(t: F) -> F {
    if t == lit::<F>(0.0) {
        lit::<F>(0.0)
    } else {
        lit::<F>(2.0).powf(lit::<F>(10.0) * (t - lit::<F>(1.0)))
    }
}

#[inline]
pub fn expo_out<F: Float>(t: F) -> F {
    if t == lit::<F>(1.0) {
        lit::<F>(1.0)
    } else {
        lit::<F>(1.0) - lit::<F>(2.0).powf(lit::<F>(-10.0) * t)
    }
}

#[inline]
pub fn expo_inout<F: Float>(t: F) -> F {
    if t == lit::<F>(0.0) {
        lit::<F>(0.0)
    } else if t == lit::<F>(1.0) {
        lit::<F>(1.0)
    } else if t < lit::<F>(0.5) {
        lit::<F>(0.5) * lit::<F>(2.0).powf(lit::<F>(20.0) * t - lit::<F>(10.0))
    } else {
        lit::<F>(-0.5) * lit::<F>(2.0).powf(lit::<F>(-20.0) * t + lit::<F>(10.0)) + lit::<F>(1.0)
    }
}

#[inline]
pub fn elastic_in<F: Float>(t: F) -> F {
    (lit::<F>(13.0) * lit::<F>(FRAC_PI_2) * t).sin()
        * lit::<F>(2.0).powf(lit::<F>(10.0) * (t - lit::<F>(1.0)))
}

#[inline]
pub fn elastic_out<F: Float>(t: F) -> F {
    (lit::<F>(-13.0) * lit::<F>(FRAC_PI_2) * (t + lit::<F>(1.0))).sin()
        * lit::<F>(2.0).powf(lit::<F>(-10.0) * t)
        + lit::<F>(1.0)
}

#[inline]
pub fn elastic_inout<F: Float>(t: F) -> F {
    if t < lit::<F>(0.5) {
        lit::<F>(0.5)
            * (lit::<F>(13.0) * lit::<F>(FRAC_PI_2) * lit::<F>(2.0) * t).sin()
            * lit::<F>(2.0).powf(lit::<F>(10.0) * (lit::<F>(2.0) * t - lit::<F>(1.0)))
    } else {
        lit::<F>(0.5)
            * ((lit::<F>(-13.0) * lit::<F>(FRAC_PI_2) * lit::<F>(2.0) * t).sin()
                * lit::<F>(2.0).powf(lit::<F>(-10.0) * (lit::<F>(2.0) * t - lit::<F>(1.0)))
                + lit::<F>(2.0))
    }
}

#[inline]
pub fn back_in<F: Float>(t: F) -> F {
    t * t * t - t * (t * lit::<F>(PI)).sin()
}

#[inline]
pub fn back_out<F: Float>(t: F) -> F {
    let f = lit::<F>(1.0) - t;
    lit::<F>(1.0) - f * f * f + f * (f * lit::<F>(PI)).sin()
}

#[inline]
pub fn back_inout<F: Float>(t: F) -> F {
    if t < lit::<F>(0.5) {
        let f = lit::<F>(2.0) * t;
        lit::<F>(0.5) * (f * f * f - f * (f * lit::<F>(PI)).sin())
    } else {
        let f = lit::<F>(2.0) - lit::<F>(2.0) * t;
        lit::<F>(0.5) * (lit::<F>(1.0) - (f * f * f - f * (f * lit::<F>(PI)).sin())) + lit::<F>(0.5)
    }
}

#[inline]
pub fn bounce_out<F: Float>(t: F) -> F {
    if t < lit::<F>(4.0 / 11.0) {
        lit::<F>(121.0 / 16.0) * t * t
    } else if t < lit::<F>(8.0 / 11.0) {
        lit::<F>(363.0 / 40.0) * t * t - lit::<F>(99.0 / 10.0) * t + lit::<F>(17.0 / 5.0)
    } else if t < lit::<F>(9.0 / 10.0) {
        lit::<F>(4356.0 / 361.0) * t * t - lit::<F>(35442.0 / 1805.0) * t
            + lit::<F>(16061.0 / 1805.0)
    } else {
        lit::<F>(54.0 / 5.0) * t * t - lit::<F>(513.0 / 25.0) * t + lit::<F>(268.0 / 25.0)
    }
}

#[inline]
pub fn bounce_in<F: Float>(t: F) -> F {
    lit::<F>(1.0) - bounce_out(lit::<F>(1.0) - t)
}

#[inline]
pub fn bounce_inout<F: Float>(t: F) -> F {
    if t < lit::<F>(0.5) {
        lit::<F>(0.5) * bounce_in(t * lit::<F>(2.0))
    } else {
        lit::<F>(0.5) * bounce_out(t * lit::<F>(2.0) - lit::<F>(1.0)) + lit::<F>(0.5)
    }
}
