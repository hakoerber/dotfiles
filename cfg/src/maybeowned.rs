use std::fmt;

pub enum MaybeOwned<'a, R>
where
    R: ToOwned + ?Sized,
    <R as ToOwned>::Owned: AsRef<R>,
{
    Owned(<R as ToOwned>::Owned),
    Ref(&'a R),
}

impl<R> AsRef<R> for MaybeOwned<'_, R>
where
    R: ToOwned + ?Sized,
    <R as ToOwned>::Owned: AsRef<R>,
{
    fn as_ref(&self) -> &R {
        match *self {
            MaybeOwned::Owned(ref owned) => <<R as ToOwned>::Owned as AsRef<R>>::as_ref(owned),
            MaybeOwned::Ref(r) => r,
        }
    }
}

impl<R> MaybeOwned<'_, R>
where
    R: ToOwned + ?Sized,
    <R as ToOwned>::Owned: AsRef<R>,
{
    pub fn into_owned(self) -> <R as ToOwned>::Owned {
        match self {
            MaybeOwned::Owned(owned) => owned,
            MaybeOwned::Ref(r) => r.to_owned(),
        }
    }
}

impl<'a, R> From<&'a R> for MaybeOwned<'a, R>
where
    R: ToOwned + ?Sized,
    <R as ToOwned>::Owned: AsRef<R>,
{
    fn from(value: &'a R) -> Self {
        Self::Ref(value)
    }
}

// conflicts with the blanket impl `From<T> for T`, because both R and F can be `Self`
// impl<'a, R, F> From<F> for MaybeOwned<'a, R>
// where
//     R: ToOwned<Owned = F> + ?Sized,
//     F: AsRef<R>,
// {
//     fn from(value: F) -> Self {
//         Self::Owned(value)
//     }
// }

impl<R> fmt::Debug for MaybeOwned<'_, R>
where
    R: ToOwned + ?Sized + fmt::Debug,
    <R as ToOwned>::Owned: AsRef<R> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Owned(ref arg0) => f.debug_tuple("Owned").field(arg0).finish(),
            Self::Ref(ref arg0) => f.debug_tuple("Ref").field(arg0).finish(),
        }
    }
}

// impl<R> ToOwned for MaybeOwned<'_, R>
// where
//     R: ToOwned + ?Sized,
//     <R as ToOwned>::Owned: AsRef<R> + Clone + Borrow<Self>,
// {
//     type Owned = Self;

//     fn to_owned(&self) -> Self {
//         match *self {
//             Self::Owned(ref arg0) => Self::Owned(arg0.clone()),
//             Self::Ref(arg0) => Self::Ref(arg0),
//         }
//     }
// }

impl<R> Clone for MaybeOwned<'_, R>
where
    R: ToOwned + ?Sized,
    <R as ToOwned>::Owned: AsRef<R> + Clone,
{
    fn clone(&self) -> Self {
        match *self {
            Self::Owned(ref arg0) => Self::Owned(arg0.clone()),
            Self::Ref(arg0) => Self::Ref(arg0),
        }
    }
}
