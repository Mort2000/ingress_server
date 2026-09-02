pub use root::*;

const _: () = ::planus::check_version_compatibility("planus-1.3.0");

/// The root namespace
///
/// Generated from these locations:
/// * File `crates/arena-protocol/schemas/public_snapshot.fbs`
#[no_implicit_prelude]
#[allow(clippy::needless_lifetimes)]
mod root {
    /// The namespace `ingress`
    ///
    /// Generated from these locations:
    /// * File `crates/arena-protocol/schemas/public_snapshot.fbs`
    pub mod ingress {
        /// The namespace `ingress.snapshot`
        ///
        /// Generated from these locations:
        /// * File `crates/arena-protocol/schemas/public_snapshot.fbs`
        pub mod snapshot {
            /// The namespace `ingress.snapshot.public`
            ///
            /// Generated from these locations:
            /// * File `crates/arena-protocol/schemas/public_snapshot.fbs`
            pub mod public {
                /// The union `EventBody` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Union `EventBody` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:17`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub enum EventBody {
                    /// The variant `PortalCaptured` in the union `EventBody`
                    PortalCaptured(::planus::alloc::boxed::Box<self::EvPortal>),

                    /// The variant `PortalAttacked` in the union `EventBody`
                    PortalAttacked(::planus::alloc::boxed::Box<self::EvAttacked>),

                    /// The variant `PortalNeutralized` in the union `EventBody`
                    PortalNeutralized(::planus::alloc::boxed::Box<self::EvPortal>),

                    /// The variant `LinkCreated` in the union `EventBody`
                    LinkCreated(::planus::alloc::boxed::Box<self::EvLink>),

                    /// The variant `LinkDestroyed` in the union `EventBody`
                    LinkDestroyed(::planus::alloc::boxed::Box<self::EvLink>),

                    /// The variant `FieldCreated` in the union `EventBody`
                    FieldCreated(::planus::alloc::boxed::Box<self::EvField>),

                    /// The variant `FieldDestroyed` in the union `EventBody`
                    FieldDestroyed(::planus::alloc::boxed::Box<self::EvField>),

                    /// The variant `Reinforcement` in the union `EventBody`
                    Reinforcement(::planus::alloc::boxed::Box<self::EvReinforce>),
                }

                impl EventBody {
                    /// Creates a [EventBodyBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> EventBodyBuilder<::planus::Uninitialized> {
                        EventBodyBuilder(::planus::Uninitialized)
                    }

                    #[inline]
                    pub fn create_portal_captured(
                        builder: &mut ::planus::Builder,
                        value: impl ::planus::WriteAsOffset<self::EvPortal>,
                    ) -> ::planus::UnionOffset<Self> {
                        ::planus::UnionOffset::new(1, value.prepare(builder).downcast())
                    }

                    #[inline]
                    pub fn create_portal_attacked(
                        builder: &mut ::planus::Builder,
                        value: impl ::planus::WriteAsOffset<self::EvAttacked>,
                    ) -> ::planus::UnionOffset<Self> {
                        ::planus::UnionOffset::new(2, value.prepare(builder).downcast())
                    }

                    #[inline]
                    pub fn create_portal_neutralized(
                        builder: &mut ::planus::Builder,
                        value: impl ::planus::WriteAsOffset<self::EvPortal>,
                    ) -> ::planus::UnionOffset<Self> {
                        ::planus::UnionOffset::new(3, value.prepare(builder).downcast())
                    }

                    #[inline]
                    pub fn create_link_created(
                        builder: &mut ::planus::Builder,
                        value: impl ::planus::WriteAsOffset<self::EvLink>,
                    ) -> ::planus::UnionOffset<Self> {
                        ::planus::UnionOffset::new(4, value.prepare(builder).downcast())
                    }

                    #[inline]
                    pub fn create_link_destroyed(
                        builder: &mut ::planus::Builder,
                        value: impl ::planus::WriteAsOffset<self::EvLink>,
                    ) -> ::planus::UnionOffset<Self> {
                        ::planus::UnionOffset::new(5, value.prepare(builder).downcast())
                    }

                    #[inline]
                    pub fn create_field_created(
                        builder: &mut ::planus::Builder,
                        value: impl ::planus::WriteAsOffset<self::EvField>,
                    ) -> ::planus::UnionOffset<Self> {
                        ::planus::UnionOffset::new(6, value.prepare(builder).downcast())
                    }

                    #[inline]
                    pub fn create_field_destroyed(
                        builder: &mut ::planus::Builder,
                        value: impl ::planus::WriteAsOffset<self::EvField>,
                    ) -> ::planus::UnionOffset<Self> {
                        ::planus::UnionOffset::new(7, value.prepare(builder).downcast())
                    }

                    #[inline]
                    pub fn create_reinforcement(
                        builder: &mut ::planus::Builder,
                        value: impl ::planus::WriteAsOffset<self::EvReinforce>,
                    ) -> ::planus::UnionOffset<Self> {
                        ::planus::UnionOffset::new(8, value.prepare(builder).downcast())
                    }
                }

                impl ::planus::WriteAsUnion<EventBody> for EventBody {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<Self> {
                        match self {
                            Self::PortalCaptured(value) => {
                                Self::create_portal_captured(builder, value)
                            }
                            Self::PortalAttacked(value) => {
                                Self::create_portal_attacked(builder, value)
                            }
                            Self::PortalNeutralized(value) => {
                                Self::create_portal_neutralized(builder, value)
                            }
                            Self::LinkCreated(value) => Self::create_link_created(builder, value),
                            Self::LinkDestroyed(value) => {
                                Self::create_link_destroyed(builder, value)
                            }
                            Self::FieldCreated(value) => Self::create_field_created(builder, value),
                            Self::FieldDestroyed(value) => {
                                Self::create_field_destroyed(builder, value)
                            }
                            Self::Reinforcement(value) => {
                                Self::create_reinforcement(builder, value)
                            }
                        }
                    }
                }

                impl ::planus::WriteAsOptionalUnion<EventBody> for EventBody {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::UnionOffset<Self>> {
                        ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                    }
                }

                /// Builder for serializing an instance of the [EventBody] type.
                ///
                /// Can be created using the [EventBody::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct EventBodyBuilder<T>(T);

                impl EventBodyBuilder<::planus::Uninitialized> {
                    /// Creates an instance of the [`PortalCaptured` variant](EventBody#variant.PortalCaptured).
                    #[inline]
                    pub fn portal_captured<T>(
                        self,
                        value: T,
                    ) -> EventBodyBuilder<::planus::Initialized<1, T>>
                    where
                        T: ::planus::WriteAsOffset<self::EvPortal>,
                    {
                        EventBodyBuilder(::planus::Initialized(value))
                    }

                    /// Creates an instance of the [`PortalAttacked` variant](EventBody#variant.PortalAttacked).
                    #[inline]
                    pub fn portal_attacked<T>(
                        self,
                        value: T,
                    ) -> EventBodyBuilder<::planus::Initialized<2, T>>
                    where
                        T: ::planus::WriteAsOffset<self::EvAttacked>,
                    {
                        EventBodyBuilder(::planus::Initialized(value))
                    }

                    /// Creates an instance of the [`PortalNeutralized` variant](EventBody#variant.PortalNeutralized).
                    #[inline]
                    pub fn portal_neutralized<T>(
                        self,
                        value: T,
                    ) -> EventBodyBuilder<::planus::Initialized<3, T>>
                    where
                        T: ::planus::WriteAsOffset<self::EvPortal>,
                    {
                        EventBodyBuilder(::planus::Initialized(value))
                    }

                    /// Creates an instance of the [`LinkCreated` variant](EventBody#variant.LinkCreated).
                    #[inline]
                    pub fn link_created<T>(
                        self,
                        value: T,
                    ) -> EventBodyBuilder<::planus::Initialized<4, T>>
                    where
                        T: ::planus::WriteAsOffset<self::EvLink>,
                    {
                        EventBodyBuilder(::planus::Initialized(value))
                    }

                    /// Creates an instance of the [`LinkDestroyed` variant](EventBody#variant.LinkDestroyed).
                    #[inline]
                    pub fn link_destroyed<T>(
                        self,
                        value: T,
                    ) -> EventBodyBuilder<::planus::Initialized<5, T>>
                    where
                        T: ::planus::WriteAsOffset<self::EvLink>,
                    {
                        EventBodyBuilder(::planus::Initialized(value))
                    }

                    /// Creates an instance of the [`FieldCreated` variant](EventBody#variant.FieldCreated).
                    #[inline]
                    pub fn field_created<T>(
                        self,
                        value: T,
                    ) -> EventBodyBuilder<::planus::Initialized<6, T>>
                    where
                        T: ::planus::WriteAsOffset<self::EvField>,
                    {
                        EventBodyBuilder(::planus::Initialized(value))
                    }

                    /// Creates an instance of the [`FieldDestroyed` variant](EventBody#variant.FieldDestroyed).
                    #[inline]
                    pub fn field_destroyed<T>(
                        self,
                        value: T,
                    ) -> EventBodyBuilder<::planus::Initialized<7, T>>
                    where
                        T: ::planus::WriteAsOffset<self::EvField>,
                    {
                        EventBodyBuilder(::planus::Initialized(value))
                    }

                    /// Creates an instance of the [`Reinforcement` variant](EventBody#variant.Reinforcement).
                    #[inline]
                    pub fn reinforcement<T>(
                        self,
                        value: T,
                    ) -> EventBodyBuilder<::planus::Initialized<8, T>>
                    where
                        T: ::planus::WriteAsOffset<self::EvReinforce>,
                    {
                        EventBodyBuilder(::planus::Initialized(value))
                    }
                }

                impl<const N: u8, T> EventBodyBuilder<::planus::Initialized<N, T>> {
                    /// Finish writing the builder to get an [UnionOffset](::planus::UnionOffset) to a serialized [EventBody].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<EventBody>
                    where
                        Self: ::planus::WriteAsUnion<EventBody>,
                    {
                        ::planus::WriteAsUnion::prepare(&self, builder)
                    }
                }

                impl<T> ::planus::WriteAsUnion<EventBody> for EventBodyBuilder<::planus::Initialized<1, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvPortal>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<EventBody> {
                        ::planus::UnionOffset::new(1, (self.0).0.prepare(builder).downcast())
                    }
                }

                impl<T> ::planus::WriteAsOptionalUnion<EventBody> for EventBodyBuilder<::planus::Initialized<1, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvPortal>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::UnionOffset<EventBody>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                    }
                }
                impl<T> ::planus::WriteAsUnion<EventBody> for EventBodyBuilder<::planus::Initialized<2, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvAttacked>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<EventBody> {
                        ::planus::UnionOffset::new(2, (self.0).0.prepare(builder).downcast())
                    }
                }

                impl<T> ::planus::WriteAsOptionalUnion<EventBody> for EventBodyBuilder<::planus::Initialized<2, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvAttacked>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::UnionOffset<EventBody>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                    }
                }
                impl<T> ::planus::WriteAsUnion<EventBody> for EventBodyBuilder<::planus::Initialized<3, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvPortal>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<EventBody> {
                        ::planus::UnionOffset::new(3, (self.0).0.prepare(builder).downcast())
                    }
                }

                impl<T> ::planus::WriteAsOptionalUnion<EventBody> for EventBodyBuilder<::planus::Initialized<3, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvPortal>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::UnionOffset<EventBody>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                    }
                }
                impl<T> ::planus::WriteAsUnion<EventBody> for EventBodyBuilder<::planus::Initialized<4, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvLink>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<EventBody> {
                        ::planus::UnionOffset::new(4, (self.0).0.prepare(builder).downcast())
                    }
                }

                impl<T> ::planus::WriteAsOptionalUnion<EventBody> for EventBodyBuilder<::planus::Initialized<4, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvLink>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::UnionOffset<EventBody>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                    }
                }
                impl<T> ::planus::WriteAsUnion<EventBody> for EventBodyBuilder<::planus::Initialized<5, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvLink>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<EventBody> {
                        ::planus::UnionOffset::new(5, (self.0).0.prepare(builder).downcast())
                    }
                }

                impl<T> ::planus::WriteAsOptionalUnion<EventBody> for EventBodyBuilder<::planus::Initialized<5, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvLink>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::UnionOffset<EventBody>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                    }
                }
                impl<T> ::planus::WriteAsUnion<EventBody> for EventBodyBuilder<::planus::Initialized<6, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvField>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<EventBody> {
                        ::planus::UnionOffset::new(6, (self.0).0.prepare(builder).downcast())
                    }
                }

                impl<T> ::planus::WriteAsOptionalUnion<EventBody> for EventBodyBuilder<::planus::Initialized<6, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvField>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::UnionOffset<EventBody>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                    }
                }
                impl<T> ::planus::WriteAsUnion<EventBody> for EventBodyBuilder<::planus::Initialized<7, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvField>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<EventBody> {
                        ::planus::UnionOffset::new(7, (self.0).0.prepare(builder).downcast())
                    }
                }

                impl<T> ::planus::WriteAsOptionalUnion<EventBody> for EventBodyBuilder<::planus::Initialized<7, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvField>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::UnionOffset<EventBody>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                    }
                }
                impl<T> ::planus::WriteAsUnion<EventBody> for EventBodyBuilder<::planus::Initialized<8, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvReinforce>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::UnionOffset<EventBody> {
                        ::planus::UnionOffset::new(8, (self.0).0.prepare(builder).downcast())
                    }
                }

                impl<T> ::planus::WriteAsOptionalUnion<EventBody> for EventBodyBuilder<::planus::Initialized<8, T>>
                where
                    T: ::planus::WriteAsOffset<self::EvReinforce>,
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::UnionOffset<EventBody>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                    }
                }

                /// Reference to a deserialized [EventBody].
                #[derive(Copy, Clone, Debug)]
                pub enum EventBodyRef<'a> {
                    PortalCaptured(self::EvPortalRef<'a>),
                    PortalAttacked(self::EvAttackedRef<'a>),
                    PortalNeutralized(self::EvPortalRef<'a>),
                    LinkCreated(self::EvLinkRef<'a>),
                    LinkDestroyed(self::EvLinkRef<'a>),
                    FieldCreated(self::EvFieldRef<'a>),
                    FieldDestroyed(self::EvFieldRef<'a>),
                    Reinforcement(self::EvReinforceRef<'a>),
                }

                impl<'a> ::core::convert::TryFrom<EventBodyRef<'a>> for EventBody {
                    type Error = ::planus::Error;

                    fn try_from(value: EventBodyRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(match value {
                            EventBodyRef::PortalCaptured(value) => {
                                Self::PortalCaptured(::planus::alloc::boxed::Box::new(
                                    ::core::convert::TryFrom::try_from(value)?,
                                ))
                            }

                            EventBodyRef::PortalAttacked(value) => {
                                Self::PortalAttacked(::planus::alloc::boxed::Box::new(
                                    ::core::convert::TryFrom::try_from(value)?,
                                ))
                            }

                            EventBodyRef::PortalNeutralized(value) => {
                                Self::PortalNeutralized(::planus::alloc::boxed::Box::new(
                                    ::core::convert::TryFrom::try_from(value)?,
                                ))
                            }

                            EventBodyRef::LinkCreated(value) => {
                                Self::LinkCreated(::planus::alloc::boxed::Box::new(
                                    ::core::convert::TryFrom::try_from(value)?,
                                ))
                            }

                            EventBodyRef::LinkDestroyed(value) => {
                                Self::LinkDestroyed(::planus::alloc::boxed::Box::new(
                                    ::core::convert::TryFrom::try_from(value)?,
                                ))
                            }

                            EventBodyRef::FieldCreated(value) => {
                                Self::FieldCreated(::planus::alloc::boxed::Box::new(
                                    ::core::convert::TryFrom::try_from(value)?,
                                ))
                            }

                            EventBodyRef::FieldDestroyed(value) => {
                                Self::FieldDestroyed(::planus::alloc::boxed::Box::new(
                                    ::core::convert::TryFrom::try_from(value)?,
                                ))
                            }

                            EventBodyRef::Reinforcement(value) => {
                                Self::Reinforcement(::planus::alloc::boxed::Box::new(
                                    ::core::convert::TryFrom::try_from(value)?,
                                ))
                            }
                        })
                    }
                }

                impl<'a> ::planus::TableReadUnion<'a> for EventBodyRef<'a> {
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        tag: u8,
                        field_offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        match tag {
                            1 => ::core::result::Result::Ok(Self::PortalCaptured(
                                ::planus::TableRead::from_buffer(buffer, field_offset)?,
                            )),
                            2 => ::core::result::Result::Ok(Self::PortalAttacked(
                                ::planus::TableRead::from_buffer(buffer, field_offset)?,
                            )),
                            3 => ::core::result::Result::Ok(Self::PortalNeutralized(
                                ::planus::TableRead::from_buffer(buffer, field_offset)?,
                            )),
                            4 => ::core::result::Result::Ok(Self::LinkCreated(
                                ::planus::TableRead::from_buffer(buffer, field_offset)?,
                            )),
                            5 => ::core::result::Result::Ok(Self::LinkDestroyed(
                                ::planus::TableRead::from_buffer(buffer, field_offset)?,
                            )),
                            6 => ::core::result::Result::Ok(Self::FieldCreated(
                                ::planus::TableRead::from_buffer(buffer, field_offset)?,
                            )),
                            7 => ::core::result::Result::Ok(Self::FieldDestroyed(
                                ::planus::TableRead::from_buffer(buffer, field_offset)?,
                            )),
                            8 => ::core::result::Result::Ok(Self::Reinforcement(
                                ::planus::TableRead::from_buffer(buffer, field_offset)?,
                            )),
                            _ => ::core::result::Result::Err(
                                ::planus::errors::ErrorKind::UnknownUnionTag { tag },
                            ),
                        }
                    }
                }

                impl<'a> ::planus::VectorReadUnion<'a> for EventBodyRef<'a> {
                    const VECTOR_NAME: &'static str = "[EventBodyRef]";
                }

                /// The table `EvPortal` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `EvPortal` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:28`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct EvPortal {
                    /// The field `faction` in the table `EvPortal`
                    pub faction: u8,
                    /// The field `agent_id` in the table `EvPortal`
                    pub agent_id: u32,
                    /// The field `portal` in the table `EvPortal`
                    pub portal: u32,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for EvPortal {
                    fn default() -> Self {
                        Self {
                            faction: 0,
                            agent_id: 0,
                            portal: 0,
                        }
                    }
                }

                impl EvPortal {
                    /// Creates a [EvPortalBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> EvPortalBuilder<()> {
                        EvPortalBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_faction: impl ::planus::WriteAsDefault<u8, u8>,
                        field_agent_id: impl ::planus::WriteAsDefault<u32, u32>,
                        field_portal: impl ::planus::WriteAsDefault<u32, u32>,
                    ) -> ::planus::Offset<Self> {
                        let prepared_faction = field_faction.prepare(builder, &0);
                        let prepared_agent_id = field_agent_id.prepare(builder, &0);
                        let prepared_portal = field_portal.prepare(builder, &0);

                        let mut table_writer: ::planus::table_writer::TableWriter<10> =
                            ::core::default::Default::default();
                        if prepared_agent_id.is_some() {
                            table_writer.write_entry::<u32>(1);
                        }
                        if prepared_portal.is_some() {
                            table_writer.write_entry::<u32>(2);
                        }
                        if prepared_faction.is_some() {
                            table_writer.write_entry::<u8>(0);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_agent_id) =
                                    prepared_agent_id
                                {
                                    object_writer.write::<_, _, 4>(&prepared_agent_id);
                                }
                                if let ::core::option::Option::Some(prepared_portal) =
                                    prepared_portal
                                {
                                    object_writer.write::<_, _, 4>(&prepared_portal);
                                }
                                if let ::core::option::Option::Some(prepared_faction) =
                                    prepared_faction
                                {
                                    object_writer.write::<_, _, 1>(&prepared_faction);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<EvPortal>> for EvPortal {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvPortal> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<EvPortal>> for EvPortal {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvPortal>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<EvPortal> for EvPortal {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvPortal> {
                        EvPortal::create(builder, self.faction, self.agent_id, self.portal)
                    }
                }

                /// Builder for serializing an instance of the [EvPortal] type.
                ///
                /// Can be created using the [EvPortal::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct EvPortalBuilder<State>(State);

                impl EvPortalBuilder<()> {
                    /// Setter for the [`faction` field](EvPortal#structfield.faction).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction<T0>(self, value: T0) -> EvPortalBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u8, u8>,
                    {
                        EvPortalBuilder((value,))
                    }

                    /// Sets the [`faction` field](EvPortal#structfield.faction) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction_as_default(self) -> EvPortalBuilder<(::planus::DefaultValue,)> {
                        self.faction(::planus::DefaultValue)
                    }
                }

                impl<T0> EvPortalBuilder<(T0,)> {
                    /// Setter for the [`agent_id` field](EvPortal#structfield.agent_id).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agent_id<T1>(self, value: T1) -> EvPortalBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0,) = self.0;
                        EvPortalBuilder((v0, value))
                    }

                    /// Sets the [`agent_id` field](EvPortal#structfield.agent_id) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agent_id_as_default(
                        self,
                    ) -> EvPortalBuilder<(T0, ::planus::DefaultValue)> {
                        self.agent_id(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> EvPortalBuilder<(T0, T1)> {
                    /// Setter for the [`portal` field](EvPortal#structfield.portal).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn portal<T2>(self, value: T2) -> EvPortalBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1) = self.0;
                        EvPortalBuilder((v0, v1, value))
                    }

                    /// Sets the [`portal` field](EvPortal#structfield.portal) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn portal_as_default(
                        self,
                    ) -> EvPortalBuilder<(T0, T1, ::planus::DefaultValue)> {
                        self.portal(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> EvPortalBuilder<(T0, T1, T2)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [EvPortal].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvPortal>
                    where
                        Self: ::planus::WriteAsOffset<EvPortal>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAs<::planus::Offset<EvPortal>> for EvPortalBuilder<(T0, T1, T2)>
                {
                    type Prepared = ::planus::Offset<EvPortal>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvPortal> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOptional<::planus::Offset<EvPortal>>
                    for EvPortalBuilder<(T0, T1, T2)>
                {
                    type Prepared = ::planus::Offset<EvPortal>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvPortal>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOffset<EvPortal> for EvPortalBuilder<(T0, T1, T2)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvPortal> {
                        let (v0, v1, v2) = &self.0;
                        EvPortal::create(builder, v0, v1, v2)
                    }
                }

                /// Reference to a deserialized [EvPortal].
                #[derive(Copy, Clone)]
                pub struct EvPortalRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

                impl<'a> EvPortalRef<'a> {
                    /// Getter for the [`faction` field](EvPortal#structfield.faction).
                    #[inline]
                    pub fn faction(&self) -> ::planus::Result<u8> {
                        ::core::result::Result::Ok(
                            self.0.access(0, "EvPortal", "faction")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`agent_id` field](EvPortal#structfield.agent_id).
                    #[inline]
                    pub fn agent_id(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "EvPortal", "agent_id")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`portal` field](EvPortal#structfield.portal).
                    #[inline]
                    pub fn portal(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(2, "EvPortal", "portal")?.unwrap_or(0),
                        )
                    }
                }

                impl<'a> ::core::fmt::Debug for EvPortalRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("EvPortalRef");
                        f.field("faction", &self.faction());
                        f.field("agent_id", &self.agent_id());
                        f.field("portal", &self.portal());
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<EvPortalRef<'a>> for EvPortal {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: EvPortalRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            faction: ::core::convert::TryInto::try_into(value.faction()?)?,
                            agent_id: ::core::convert::TryInto::try_into(value.agent_id()?)?,
                            portal: ::core::convert::TryInto::try_into(value.portal()?)?,
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for EvPortalRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for EvPortalRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[EvPortalRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<EvPortal>> for EvPortal {
                    type Value = ::planus::Offset<EvPortal>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<EvPortal>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for EvPortalRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[EvPortalRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `EvAttacked` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `EvAttacked` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:34`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct EvAttacked {
                    /// The field `faction` in the table `EvAttacked`
                    pub faction: u8,
                    /// The field `agent_id` in the table `EvAttacked`
                    pub agent_id: u32,
                    /// The field `portal` in the table `EvAttacked`
                    pub portal: u32,
                    /// The field `damage` in the table `EvAttacked`
                    pub damage: u32,
                    /// The field `hp_after` in the table `EvAttacked`
                    pub hp_after: u32,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for EvAttacked {
                    fn default() -> Self {
                        Self {
                            faction: 0,
                            agent_id: 0,
                            portal: 0,
                            damage: 0,
                            hp_after: 0,
                        }
                    }
                }

                impl EvAttacked {
                    /// Creates a [EvAttackedBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> EvAttackedBuilder<()> {
                        EvAttackedBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_faction: impl ::planus::WriteAsDefault<u8, u8>,
                        field_agent_id: impl ::planus::WriteAsDefault<u32, u32>,
                        field_portal: impl ::planus::WriteAsDefault<u32, u32>,
                        field_damage: impl ::planus::WriteAsDefault<u32, u32>,
                        field_hp_after: impl ::planus::WriteAsDefault<u32, u32>,
                    ) -> ::planus::Offset<Self> {
                        let prepared_faction = field_faction.prepare(builder, &0);
                        let prepared_agent_id = field_agent_id.prepare(builder, &0);
                        let prepared_portal = field_portal.prepare(builder, &0);
                        let prepared_damage = field_damage.prepare(builder, &0);
                        let prepared_hp_after = field_hp_after.prepare(builder, &0);

                        let mut table_writer: ::planus::table_writer::TableWriter<14> =
                            ::core::default::Default::default();
                        if prepared_agent_id.is_some() {
                            table_writer.write_entry::<u32>(1);
                        }
                        if prepared_portal.is_some() {
                            table_writer.write_entry::<u32>(2);
                        }
                        if prepared_damage.is_some() {
                            table_writer.write_entry::<u32>(3);
                        }
                        if prepared_hp_after.is_some() {
                            table_writer.write_entry::<u32>(4);
                        }
                        if prepared_faction.is_some() {
                            table_writer.write_entry::<u8>(0);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_agent_id) =
                                    prepared_agent_id
                                {
                                    object_writer.write::<_, _, 4>(&prepared_agent_id);
                                }
                                if let ::core::option::Option::Some(prepared_portal) =
                                    prepared_portal
                                {
                                    object_writer.write::<_, _, 4>(&prepared_portal);
                                }
                                if let ::core::option::Option::Some(prepared_damage) =
                                    prepared_damage
                                {
                                    object_writer.write::<_, _, 4>(&prepared_damage);
                                }
                                if let ::core::option::Option::Some(prepared_hp_after) =
                                    prepared_hp_after
                                {
                                    object_writer.write::<_, _, 4>(&prepared_hp_after);
                                }
                                if let ::core::option::Option::Some(prepared_faction) =
                                    prepared_faction
                                {
                                    object_writer.write::<_, _, 1>(&prepared_faction);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<EvAttacked>> for EvAttacked {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvAttacked> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<EvAttacked>> for EvAttacked {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvAttacked>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<EvAttacked> for EvAttacked {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvAttacked> {
                        EvAttacked::create(
                            builder,
                            self.faction,
                            self.agent_id,
                            self.portal,
                            self.damage,
                            self.hp_after,
                        )
                    }
                }

                /// Builder for serializing an instance of the [EvAttacked] type.
                ///
                /// Can be created using the [EvAttacked::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct EvAttackedBuilder<State>(State);

                impl EvAttackedBuilder<()> {
                    /// Setter for the [`faction` field](EvAttacked#structfield.faction).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction<T0>(self, value: T0) -> EvAttackedBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u8, u8>,
                    {
                        EvAttackedBuilder((value,))
                    }

                    /// Sets the [`faction` field](EvAttacked#structfield.faction) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction_as_default(
                        self,
                    ) -> EvAttackedBuilder<(::planus::DefaultValue,)> {
                        self.faction(::planus::DefaultValue)
                    }
                }

                impl<T0> EvAttackedBuilder<(T0,)> {
                    /// Setter for the [`agent_id` field](EvAttacked#structfield.agent_id).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agent_id<T1>(self, value: T1) -> EvAttackedBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0,) = self.0;
                        EvAttackedBuilder((v0, value))
                    }

                    /// Sets the [`agent_id` field](EvAttacked#structfield.agent_id) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agent_id_as_default(
                        self,
                    ) -> EvAttackedBuilder<(T0, ::planus::DefaultValue)> {
                        self.agent_id(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> EvAttackedBuilder<(T0, T1)> {
                    /// Setter for the [`portal` field](EvAttacked#structfield.portal).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn portal<T2>(self, value: T2) -> EvAttackedBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1) = self.0;
                        EvAttackedBuilder((v0, v1, value))
                    }

                    /// Sets the [`portal` field](EvAttacked#structfield.portal) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn portal_as_default(
                        self,
                    ) -> EvAttackedBuilder<(T0, T1, ::planus::DefaultValue)> {
                        self.portal(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> EvAttackedBuilder<(T0, T1, T2)> {
                    /// Setter for the [`damage` field](EvAttacked#structfield.damage).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn damage<T3>(self, value: T3) -> EvAttackedBuilder<(T0, T1, T2, T3)>
                    where
                        T3: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1, v2) = self.0;
                        EvAttackedBuilder((v0, v1, v2, value))
                    }

                    /// Sets the [`damage` field](EvAttacked#structfield.damage) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn damage_as_default(
                        self,
                    ) -> EvAttackedBuilder<(T0, T1, T2, ::planus::DefaultValue)>
                    {
                        self.damage(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3> EvAttackedBuilder<(T0, T1, T2, T3)> {
                    /// Setter for the [`hp_after` field](EvAttacked#structfield.hp_after).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn hp_after<T4>(self, value: T4) -> EvAttackedBuilder<(T0, T1, T2, T3, T4)>
                    where
                        T4: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1, v2, v3) = self.0;
                        EvAttackedBuilder((v0, v1, v2, v3, value))
                    }

                    /// Sets the [`hp_after` field](EvAttacked#structfield.hp_after) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn hp_after_as_default(
                        self,
                    ) -> EvAttackedBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)>
                    {
                        self.hp_after(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4> EvAttackedBuilder<(T0, T1, T2, T3, T4)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [EvAttacked].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvAttacked>
                    where
                        Self: ::planus::WriteAsOffset<EvAttacked>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAs<::planus::Offset<EvAttacked>>
                    for EvAttackedBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<EvAttacked>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvAttacked> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOptional<::planus::Offset<EvAttacked>>
                    for EvAttackedBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<EvAttacked>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvAttacked>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOffset<EvAttacked> for EvAttackedBuilder<(T0, T1, T2, T3, T4)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvAttacked> {
                        let (v0, v1, v2, v3, v4) = &self.0;
                        EvAttacked::create(builder, v0, v1, v2, v3, v4)
                    }
                }

                /// Reference to a deserialized [EvAttacked].
                #[derive(Copy, Clone)]
                pub struct EvAttackedRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

                impl<'a> EvAttackedRef<'a> {
                    /// Getter for the [`faction` field](EvAttacked#structfield.faction).
                    #[inline]
                    pub fn faction(&self) -> ::planus::Result<u8> {
                        ::core::result::Result::Ok(
                            self.0.access(0, "EvAttacked", "faction")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`agent_id` field](EvAttacked#structfield.agent_id).
                    #[inline]
                    pub fn agent_id(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "EvAttacked", "agent_id")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`portal` field](EvAttacked#structfield.portal).
                    #[inline]
                    pub fn portal(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(2, "EvAttacked", "portal")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`damage` field](EvAttacked#structfield.damage).
                    #[inline]
                    pub fn damage(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(3, "EvAttacked", "damage")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`hp_after` field](EvAttacked#structfield.hp_after).
                    #[inline]
                    pub fn hp_after(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(4, "EvAttacked", "hp_after")?.unwrap_or(0),
                        )
                    }
                }

                impl<'a> ::core::fmt::Debug for EvAttackedRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("EvAttackedRef");
                        f.field("faction", &self.faction());
                        f.field("agent_id", &self.agent_id());
                        f.field("portal", &self.portal());
                        f.field("damage", &self.damage());
                        f.field("hp_after", &self.hp_after());
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<EvAttackedRef<'a>> for EvAttacked {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: EvAttackedRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            faction: ::core::convert::TryInto::try_into(value.faction()?)?,
                            agent_id: ::core::convert::TryInto::try_into(value.agent_id()?)?,
                            portal: ::core::convert::TryInto::try_into(value.portal()?)?,
                            damage: ::core::convert::TryInto::try_into(value.damage()?)?,
                            hp_after: ::core::convert::TryInto::try_into(value.hp_after()?)?,
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for EvAttackedRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for EvAttackedRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[EvAttackedRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<EvAttacked>> for EvAttacked {
                    type Value = ::planus::Offset<EvAttacked>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<EvAttacked>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for EvAttackedRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[EvAttackedRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `EvLink` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `EvLink` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:42`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct EvLink {
                    /// The field `faction` in the table `EvLink`
                    pub faction: u8,
                    /// The field `agent_id` in the table `EvLink`
                    pub agent_id: u32,
                    /// The field `a` in the table `EvLink`
                    pub a: u32,
                    /// The field `b` in the table `EvLink`
                    pub b: u32,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for EvLink {
                    fn default() -> Self {
                        Self {
                            faction: 0,
                            agent_id: 0,
                            a: 0,
                            b: 0,
                        }
                    }
                }

                impl EvLink {
                    /// Creates a [EvLinkBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> EvLinkBuilder<()> {
                        EvLinkBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_faction: impl ::planus::WriteAsDefault<u8, u8>,
                        field_agent_id: impl ::planus::WriteAsDefault<u32, u32>,
                        field_a: impl ::planus::WriteAsDefault<u32, u32>,
                        field_b: impl ::planus::WriteAsDefault<u32, u32>,
                    ) -> ::planus::Offset<Self> {
                        let prepared_faction = field_faction.prepare(builder, &0);
                        let prepared_agent_id = field_agent_id.prepare(builder, &0);
                        let prepared_a = field_a.prepare(builder, &0);
                        let prepared_b = field_b.prepare(builder, &0);

                        let mut table_writer: ::planus::table_writer::TableWriter<12> =
                            ::core::default::Default::default();
                        if prepared_agent_id.is_some() {
                            table_writer.write_entry::<u32>(1);
                        }
                        if prepared_a.is_some() {
                            table_writer.write_entry::<u32>(2);
                        }
                        if prepared_b.is_some() {
                            table_writer.write_entry::<u32>(3);
                        }
                        if prepared_faction.is_some() {
                            table_writer.write_entry::<u8>(0);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_agent_id) =
                                    prepared_agent_id
                                {
                                    object_writer.write::<_, _, 4>(&prepared_agent_id);
                                }
                                if let ::core::option::Option::Some(prepared_a) = prepared_a {
                                    object_writer.write::<_, _, 4>(&prepared_a);
                                }
                                if let ::core::option::Option::Some(prepared_b) = prepared_b {
                                    object_writer.write::<_, _, 4>(&prepared_b);
                                }
                                if let ::core::option::Option::Some(prepared_faction) =
                                    prepared_faction
                                {
                                    object_writer.write::<_, _, 1>(&prepared_faction);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<EvLink>> for EvLink {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<EvLink> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<EvLink>> for EvLink {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvLink>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<EvLink> for EvLink {
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<EvLink> {
                        EvLink::create(builder, self.faction, self.agent_id, self.a, self.b)
                    }
                }

                /// Builder for serializing an instance of the [EvLink] type.
                ///
                /// Can be created using the [EvLink::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct EvLinkBuilder<State>(State);

                impl EvLinkBuilder<()> {
                    /// Setter for the [`faction` field](EvLink#structfield.faction).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction<T0>(self, value: T0) -> EvLinkBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u8, u8>,
                    {
                        EvLinkBuilder((value,))
                    }

                    /// Sets the [`faction` field](EvLink#structfield.faction) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction_as_default(self) -> EvLinkBuilder<(::planus::DefaultValue,)> {
                        self.faction(::planus::DefaultValue)
                    }
                }

                impl<T0> EvLinkBuilder<(T0,)> {
                    /// Setter for the [`agent_id` field](EvLink#structfield.agent_id).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agent_id<T1>(self, value: T1) -> EvLinkBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0,) = self.0;
                        EvLinkBuilder((v0, value))
                    }

                    /// Sets the [`agent_id` field](EvLink#structfield.agent_id) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agent_id_as_default(
                        self,
                    ) -> EvLinkBuilder<(T0, ::planus::DefaultValue)> {
                        self.agent_id(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> EvLinkBuilder<(T0, T1)> {
                    /// Setter for the [`a` field](EvLink#structfield.a).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn a<T2>(self, value: T2) -> EvLinkBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1) = self.0;
                        EvLinkBuilder((v0, v1, value))
                    }

                    /// Sets the [`a` field](EvLink#structfield.a) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn a_as_default(self) -> EvLinkBuilder<(T0, T1, ::planus::DefaultValue)> {
                        self.a(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> EvLinkBuilder<(T0, T1, T2)> {
                    /// Setter for the [`b` field](EvLink#structfield.b).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn b<T3>(self, value: T3) -> EvLinkBuilder<(T0, T1, T2, T3)>
                    where
                        T3: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1, v2) = self.0;
                        EvLinkBuilder((v0, v1, v2, value))
                    }

                    /// Sets the [`b` field](EvLink#structfield.b) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn b_as_default(
                        self,
                    ) -> EvLinkBuilder<(T0, T1, T2, ::planus::DefaultValue)> {
                        self.b(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3> EvLinkBuilder<(T0, T1, T2, T3)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [EvLink].
                    #[inline]
                    pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<EvLink>
                    where
                        Self: ::planus::WriteAsOffset<EvLink>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAs<::planus::Offset<EvLink>> for EvLinkBuilder<(T0, T1, T2, T3)>
                {
                    type Prepared = ::planus::Offset<EvLink>;

                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<EvLink> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOptional<::planus::Offset<EvLink>>
                    for EvLinkBuilder<(T0, T1, T2, T3)>
                {
                    type Prepared = ::planus::Offset<EvLink>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvLink>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOffset<EvLink> for EvLinkBuilder<(T0, T1, T2, T3)>
                {
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<EvLink> {
                        let (v0, v1, v2, v3) = &self.0;
                        EvLink::create(builder, v0, v1, v2, v3)
                    }
                }

                /// Reference to a deserialized [EvLink].
                #[derive(Copy, Clone)]
                pub struct EvLinkRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

                impl<'a> EvLinkRef<'a> {
                    /// Getter for the [`faction` field](EvLink#structfield.faction).
                    #[inline]
                    pub fn faction(&self) -> ::planus::Result<u8> {
                        ::core::result::Result::Ok(
                            self.0.access(0, "EvLink", "faction")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`agent_id` field](EvLink#structfield.agent_id).
                    #[inline]
                    pub fn agent_id(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "EvLink", "agent_id")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`a` field](EvLink#structfield.a).
                    #[inline]
                    pub fn a(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(self.0.access(2, "EvLink", "a")?.unwrap_or(0))
                    }

                    /// Getter for the [`b` field](EvLink#structfield.b).
                    #[inline]
                    pub fn b(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(self.0.access(3, "EvLink", "b")?.unwrap_or(0))
                    }
                }

                impl<'a> ::core::fmt::Debug for EvLinkRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("EvLinkRef");
                        f.field("faction", &self.faction());
                        f.field("agent_id", &self.agent_id());
                        f.field("a", &self.a());
                        f.field("b", &self.b());
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<EvLinkRef<'a>> for EvLink {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: EvLinkRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            faction: ::core::convert::TryInto::try_into(value.faction()?)?,
                            agent_id: ::core::convert::TryInto::try_into(value.agent_id()?)?,
                            a: ::core::convert::TryInto::try_into(value.a()?)?,
                            b: ::core::convert::TryInto::try_into(value.b()?)?,
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for EvLinkRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for EvLinkRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[EvLinkRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<EvLink>> for EvLink {
                    type Value = ::planus::Offset<EvLink>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<EvLink>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for EvLinkRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[EvLinkRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `EvField` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `EvField` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:49`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct EvField {
                    /// The field `faction` in the table `EvField`
                    pub faction: u8,
                    /// The field `agent_id` in the table `EvField`
                    pub agent_id: u32,
                    /// The field `a` in the table `EvField`
                    pub a: u32,
                    /// The field `b` in the table `EvField`
                    pub b: u32,
                    /// The field `c` in the table `EvField`
                    pub c: u32,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for EvField {
                    fn default() -> Self {
                        Self {
                            faction: 0,
                            agent_id: 0,
                            a: 0,
                            b: 0,
                            c: 0,
                        }
                    }
                }

                impl EvField {
                    /// Creates a [EvFieldBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> EvFieldBuilder<()> {
                        EvFieldBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_faction: impl ::planus::WriteAsDefault<u8, u8>,
                        field_agent_id: impl ::planus::WriteAsDefault<u32, u32>,
                        field_a: impl ::planus::WriteAsDefault<u32, u32>,
                        field_b: impl ::planus::WriteAsDefault<u32, u32>,
                        field_c: impl ::planus::WriteAsDefault<u32, u32>,
                    ) -> ::planus::Offset<Self> {
                        let prepared_faction = field_faction.prepare(builder, &0);
                        let prepared_agent_id = field_agent_id.prepare(builder, &0);
                        let prepared_a = field_a.prepare(builder, &0);
                        let prepared_b = field_b.prepare(builder, &0);
                        let prepared_c = field_c.prepare(builder, &0);

                        let mut table_writer: ::planus::table_writer::TableWriter<14> =
                            ::core::default::Default::default();
                        if prepared_agent_id.is_some() {
                            table_writer.write_entry::<u32>(1);
                        }
                        if prepared_a.is_some() {
                            table_writer.write_entry::<u32>(2);
                        }
                        if prepared_b.is_some() {
                            table_writer.write_entry::<u32>(3);
                        }
                        if prepared_c.is_some() {
                            table_writer.write_entry::<u32>(4);
                        }
                        if prepared_faction.is_some() {
                            table_writer.write_entry::<u8>(0);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_agent_id) =
                                    prepared_agent_id
                                {
                                    object_writer.write::<_, _, 4>(&prepared_agent_id);
                                }
                                if let ::core::option::Option::Some(prepared_a) = prepared_a {
                                    object_writer.write::<_, _, 4>(&prepared_a);
                                }
                                if let ::core::option::Option::Some(prepared_b) = prepared_b {
                                    object_writer.write::<_, _, 4>(&prepared_b);
                                }
                                if let ::core::option::Option::Some(prepared_c) = prepared_c {
                                    object_writer.write::<_, _, 4>(&prepared_c);
                                }
                                if let ::core::option::Option::Some(prepared_faction) =
                                    prepared_faction
                                {
                                    object_writer.write::<_, _, 1>(&prepared_faction);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<EvField>> for EvField {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvField> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<EvField>> for EvField {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvField>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<EvField> for EvField {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvField> {
                        EvField::create(
                            builder,
                            self.faction,
                            self.agent_id,
                            self.a,
                            self.b,
                            self.c,
                        )
                    }
                }

                /// Builder for serializing an instance of the [EvField] type.
                ///
                /// Can be created using the [EvField::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct EvFieldBuilder<State>(State);

                impl EvFieldBuilder<()> {
                    /// Setter for the [`faction` field](EvField#structfield.faction).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction<T0>(self, value: T0) -> EvFieldBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u8, u8>,
                    {
                        EvFieldBuilder((value,))
                    }

                    /// Sets the [`faction` field](EvField#structfield.faction) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction_as_default(self) -> EvFieldBuilder<(::planus::DefaultValue,)> {
                        self.faction(::planus::DefaultValue)
                    }
                }

                impl<T0> EvFieldBuilder<(T0,)> {
                    /// Setter for the [`agent_id` field](EvField#structfield.agent_id).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agent_id<T1>(self, value: T1) -> EvFieldBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0,) = self.0;
                        EvFieldBuilder((v0, value))
                    }

                    /// Sets the [`agent_id` field](EvField#structfield.agent_id) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agent_id_as_default(
                        self,
                    ) -> EvFieldBuilder<(T0, ::planus::DefaultValue)> {
                        self.agent_id(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> EvFieldBuilder<(T0, T1)> {
                    /// Setter for the [`a` field](EvField#structfield.a).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn a<T2>(self, value: T2) -> EvFieldBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1) = self.0;
                        EvFieldBuilder((v0, v1, value))
                    }

                    /// Sets the [`a` field](EvField#structfield.a) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn a_as_default(self) -> EvFieldBuilder<(T0, T1, ::planus::DefaultValue)> {
                        self.a(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> EvFieldBuilder<(T0, T1, T2)> {
                    /// Setter for the [`b` field](EvField#structfield.b).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn b<T3>(self, value: T3) -> EvFieldBuilder<(T0, T1, T2, T3)>
                    where
                        T3: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1, v2) = self.0;
                        EvFieldBuilder((v0, v1, v2, value))
                    }

                    /// Sets the [`b` field](EvField#structfield.b) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn b_as_default(
                        self,
                    ) -> EvFieldBuilder<(T0, T1, T2, ::planus::DefaultValue)> {
                        self.b(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3> EvFieldBuilder<(T0, T1, T2, T3)> {
                    /// Setter for the [`c` field](EvField#structfield.c).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn c<T4>(self, value: T4) -> EvFieldBuilder<(T0, T1, T2, T3, T4)>
                    where
                        T4: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1, v2, v3) = self.0;
                        EvFieldBuilder((v0, v1, v2, v3, value))
                    }

                    /// Sets the [`c` field](EvField#structfield.c) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn c_as_default(
                        self,
                    ) -> EvFieldBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)>
                    {
                        self.c(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4> EvFieldBuilder<(T0, T1, T2, T3, T4)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [EvField].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvField>
                    where
                        Self: ::planus::WriteAsOffset<EvField>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAs<::planus::Offset<EvField>>
                    for EvFieldBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<EvField>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvField> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOptional<::planus::Offset<EvField>>
                    for EvFieldBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<EvField>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvField>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOffset<EvField> for EvFieldBuilder<(T0, T1, T2, T3, T4)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvField> {
                        let (v0, v1, v2, v3, v4) = &self.0;
                        EvField::create(builder, v0, v1, v2, v3, v4)
                    }
                }

                /// Reference to a deserialized [EvField].
                #[derive(Copy, Clone)]
                pub struct EvFieldRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

                impl<'a> EvFieldRef<'a> {
                    /// Getter for the [`faction` field](EvField#structfield.faction).
                    #[inline]
                    pub fn faction(&self) -> ::planus::Result<u8> {
                        ::core::result::Result::Ok(
                            self.0.access(0, "EvField", "faction")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`agent_id` field](EvField#structfield.agent_id).
                    #[inline]
                    pub fn agent_id(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "EvField", "agent_id")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`a` field](EvField#structfield.a).
                    #[inline]
                    pub fn a(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(self.0.access(2, "EvField", "a")?.unwrap_or(0))
                    }

                    /// Getter for the [`b` field](EvField#structfield.b).
                    #[inline]
                    pub fn b(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(self.0.access(3, "EvField", "b")?.unwrap_or(0))
                    }

                    /// Getter for the [`c` field](EvField#structfield.c).
                    #[inline]
                    pub fn c(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(self.0.access(4, "EvField", "c")?.unwrap_or(0))
                    }
                }

                impl<'a> ::core::fmt::Debug for EvFieldRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("EvFieldRef");
                        f.field("faction", &self.faction());
                        f.field("agent_id", &self.agent_id());
                        f.field("a", &self.a());
                        f.field("b", &self.b());
                        f.field("c", &self.c());
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<EvFieldRef<'a>> for EvField {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: EvFieldRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            faction: ::core::convert::TryInto::try_into(value.faction()?)?,
                            agent_id: ::core::convert::TryInto::try_into(value.agent_id()?)?,
                            a: ::core::convert::TryInto::try_into(value.a()?)?,
                            b: ::core::convert::TryInto::try_into(value.b()?)?,
                            c: ::core::convert::TryInto::try_into(value.c()?)?,
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for EvFieldRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for EvFieldRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[EvFieldRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<EvField>> for EvField {
                    type Value = ::planus::Offset<EvField>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<EvField>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for EvFieldRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[EvFieldRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `EvReinforce` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `EvReinforce` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:57`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct EvReinforce {
                    /// The field `index` in the table `EvReinforce`
                    pub index: u32,
                    /// The field `x` in the table `EvReinforce`
                    pub x: i32,
                    /// The field `y` in the table `EvReinforce`
                    pub y: i32,
                    /// The field `count` in the table `EvReinforce`
                    pub count: u32,
                    /// The field `first_agent_id` in the table `EvReinforce`
                    pub first_agent_id: u32,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for EvReinforce {
                    fn default() -> Self {
                        Self {
                            index: 0,
                            x: 0,
                            y: 0,
                            count: 0,
                            first_agent_id: 0,
                        }
                    }
                }

                impl EvReinforce {
                    /// Creates a [EvReinforceBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> EvReinforceBuilder<()> {
                        EvReinforceBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_index: impl ::planus::WriteAsDefault<u32, u32>,
                        field_x: impl ::planus::WriteAsDefault<i32, i32>,
                        field_y: impl ::planus::WriteAsDefault<i32, i32>,
                        field_count: impl ::planus::WriteAsDefault<u32, u32>,
                        field_first_agent_id: impl ::planus::WriteAsDefault<u32, u32>,
                    ) -> ::planus::Offset<Self> {
                        let prepared_index = field_index.prepare(builder, &0);
                        let prepared_x = field_x.prepare(builder, &0);
                        let prepared_y = field_y.prepare(builder, &0);
                        let prepared_count = field_count.prepare(builder, &0);
                        let prepared_first_agent_id = field_first_agent_id.prepare(builder, &0);

                        let mut table_writer: ::planus::table_writer::TableWriter<14> =
                            ::core::default::Default::default();
                        if prepared_index.is_some() {
                            table_writer.write_entry::<u32>(0);
                        }
                        if prepared_x.is_some() {
                            table_writer.write_entry::<i32>(1);
                        }
                        if prepared_y.is_some() {
                            table_writer.write_entry::<i32>(2);
                        }
                        if prepared_count.is_some() {
                            table_writer.write_entry::<u32>(3);
                        }
                        if prepared_first_agent_id.is_some() {
                            table_writer.write_entry::<u32>(4);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_index) = prepared_index
                                {
                                    object_writer.write::<_, _, 4>(&prepared_index);
                                }
                                if let ::core::option::Option::Some(prepared_x) = prepared_x {
                                    object_writer.write::<_, _, 4>(&prepared_x);
                                }
                                if let ::core::option::Option::Some(prepared_y) = prepared_y {
                                    object_writer.write::<_, _, 4>(&prepared_y);
                                }
                                if let ::core::option::Option::Some(prepared_count) = prepared_count
                                {
                                    object_writer.write::<_, _, 4>(&prepared_count);
                                }
                                if let ::core::option::Option::Some(prepared_first_agent_id) =
                                    prepared_first_agent_id
                                {
                                    object_writer.write::<_, _, 4>(&prepared_first_agent_id);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<EvReinforce>> for EvReinforce {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvReinforce> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<EvReinforce>> for EvReinforce {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvReinforce>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<EvReinforce> for EvReinforce {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvReinforce> {
                        EvReinforce::create(
                            builder,
                            self.index,
                            self.x,
                            self.y,
                            self.count,
                            self.first_agent_id,
                        )
                    }
                }

                /// Builder for serializing an instance of the [EvReinforce] type.
                ///
                /// Can be created using the [EvReinforce::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct EvReinforceBuilder<State>(State);

                impl EvReinforceBuilder<()> {
                    /// Setter for the [`index` field](EvReinforce#structfield.index).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn index<T0>(self, value: T0) -> EvReinforceBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u32, u32>,
                    {
                        EvReinforceBuilder((value,))
                    }

                    /// Sets the [`index` field](EvReinforce#structfield.index) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn index_as_default(self) -> EvReinforceBuilder<(::planus::DefaultValue,)> {
                        self.index(::planus::DefaultValue)
                    }
                }

                impl<T0> EvReinforceBuilder<(T0,)> {
                    /// Setter for the [`x` field](EvReinforce#structfield.x).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn x<T1>(self, value: T1) -> EvReinforceBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<i32, i32>,
                    {
                        let (v0,) = self.0;
                        EvReinforceBuilder((v0, value))
                    }

                    /// Sets the [`x` field](EvReinforce#structfield.x) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn x_as_default(self) -> EvReinforceBuilder<(T0, ::planus::DefaultValue)> {
                        self.x(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> EvReinforceBuilder<(T0, T1)> {
                    /// Setter for the [`y` field](EvReinforce#structfield.y).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn y<T2>(self, value: T2) -> EvReinforceBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<i32, i32>,
                    {
                        let (v0, v1) = self.0;
                        EvReinforceBuilder((v0, v1, value))
                    }

                    /// Sets the [`y` field](EvReinforce#structfield.y) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn y_as_default(
                        self,
                    ) -> EvReinforceBuilder<(T0, T1, ::planus::DefaultValue)> {
                        self.y(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> EvReinforceBuilder<(T0, T1, T2)> {
                    /// Setter for the [`count` field](EvReinforce#structfield.count).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn count<T3>(self, value: T3) -> EvReinforceBuilder<(T0, T1, T2, T3)>
                    where
                        T3: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1, v2) = self.0;
                        EvReinforceBuilder((v0, v1, v2, value))
                    }

                    /// Sets the [`count` field](EvReinforce#structfield.count) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn count_as_default(
                        self,
                    ) -> EvReinforceBuilder<(T0, T1, T2, ::planus::DefaultValue)>
                    {
                        self.count(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3> EvReinforceBuilder<(T0, T1, T2, T3)> {
                    /// Setter for the [`first_agent_id` field](EvReinforce#structfield.first_agent_id).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn first_agent_id<T4>(
                        self,
                        value: T4,
                    ) -> EvReinforceBuilder<(T0, T1, T2, T3, T4)>
                    where
                        T4: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1, v2, v3) = self.0;
                        EvReinforceBuilder((v0, v1, v2, v3, value))
                    }

                    /// Sets the [`first_agent_id` field](EvReinforce#structfield.first_agent_id) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn first_agent_id_as_default(
                        self,
                    ) -> EvReinforceBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)>
                    {
                        self.first_agent_id(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4> EvReinforceBuilder<(T0, T1, T2, T3, T4)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [EvReinforce].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvReinforce>
                    where
                        Self: ::planus::WriteAsOffset<EvReinforce>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<i32, i32>,
                    T2: ::planus::WriteAsDefault<i32, i32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAs<::planus::Offset<EvReinforce>>
                    for EvReinforceBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<EvReinforce>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvReinforce> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<i32, i32>,
                    T2: ::planus::WriteAsDefault<i32, i32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOptional<::planus::Offset<EvReinforce>>
                    for EvReinforceBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<EvReinforce>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EvReinforce>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<i32, i32>,
                    T2: ::planus::WriteAsDefault<i32, i32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOffset<EvReinforce>
                    for EvReinforceBuilder<(T0, T1, T2, T3, T4)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EvReinforce> {
                        let (v0, v1, v2, v3, v4) = &self.0;
                        EvReinforce::create(builder, v0, v1, v2, v3, v4)
                    }
                }

                /// Reference to a deserialized [EvReinforce].
                #[derive(Copy, Clone)]
                pub struct EvReinforceRef<'a>(
                    #[allow(dead_code)] ::planus::table_reader::Table<'a>,
                );

                impl<'a> EvReinforceRef<'a> {
                    /// Getter for the [`index` field](EvReinforce#structfield.index).
                    #[inline]
                    pub fn index(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(0, "EvReinforce", "index")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`x` field](EvReinforce#structfield.x).
                    #[inline]
                    pub fn x(&self) -> ::planus::Result<i32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "EvReinforce", "x")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`y` field](EvReinforce#structfield.y).
                    #[inline]
                    pub fn y(&self) -> ::planus::Result<i32> {
                        ::core::result::Result::Ok(
                            self.0.access(2, "EvReinforce", "y")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`count` field](EvReinforce#structfield.count).
                    #[inline]
                    pub fn count(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(3, "EvReinforce", "count")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`first_agent_id` field](EvReinforce#structfield.first_agent_id).
                    #[inline]
                    pub fn first_agent_id(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(4, "EvReinforce", "first_agent_id")?
                                .unwrap_or(0),
                        )
                    }
                }

                impl<'a> ::core::fmt::Debug for EvReinforceRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("EvReinforceRef");
                        f.field("index", &self.index());
                        f.field("x", &self.x());
                        f.field("y", &self.y());
                        f.field("count", &self.count());
                        f.field("first_agent_id", &self.first_agent_id());
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<EvReinforceRef<'a>> for EvReinforce {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: EvReinforceRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            index: ::core::convert::TryInto::try_into(value.index()?)?,
                            x: ::core::convert::TryInto::try_into(value.x()?)?,
                            y: ::core::convert::TryInto::try_into(value.y()?)?,
                            count: ::core::convert::TryInto::try_into(value.count()?)?,
                            first_agent_id: ::core::convert::TryInto::try_into(
                                value.first_agent_id()?,
                            )?,
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for EvReinforceRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for EvReinforceRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[EvReinforceRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<EvReinforce>> for EvReinforce {
                    type Value = ::planus::Offset<EvReinforce>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<EvReinforce>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for EvReinforceRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[EvReinforceRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `EventEntry` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `EventEntry` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:65`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct EventEntry {
                    /// The field `seq` in the table `EventEntry`
                    pub seq: u64,
                    /// The field `round` in the table `EventEntry`
                    pub round: u32,
                    /// The field `body` in the table `EventEntry`
                    pub body: ::core::option::Option<self::EventBody>,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for EventEntry {
                    fn default() -> Self {
                        Self {
                            seq: 0,
                            round: 0,
                            body: ::core::default::Default::default(),
                        }
                    }
                }

                impl EventEntry {
                    /// Creates a [EventEntryBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> EventEntryBuilder<()> {
                        EventEntryBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_seq: impl ::planus::WriteAsDefault<u64, u64>,
                        field_round: impl ::planus::WriteAsDefault<u32, u32>,
                        field_body: impl ::planus::WriteAsOptionalUnion<self::EventBody>,
                    ) -> ::planus::Offset<Self> {
                        let prepared_seq = field_seq.prepare(builder, &0);
                        let prepared_round = field_round.prepare(builder, &0);
                        let prepared_body = field_body.prepare(builder);

                        let mut table_writer: ::planus::table_writer::TableWriter<12> =
                            ::core::default::Default::default();
                        if prepared_seq.is_some() {
                            table_writer.write_entry::<u64>(0);
                        }
                        if prepared_round.is_some() {
                            table_writer.write_entry::<u32>(1);
                        }
                        if prepared_body.is_some() {
                            table_writer.write_entry::<::planus::Offset<self::EventBody>>(3);
                        }
                        if prepared_body.is_some() {
                            table_writer.write_entry::<u8>(2);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_seq) = prepared_seq {
                                    object_writer.write::<_, _, 8>(&prepared_seq);
                                }
                                if let ::core::option::Option::Some(prepared_round) = prepared_round
                                {
                                    object_writer.write::<_, _, 4>(&prepared_round);
                                }
                                if let ::core::option::Option::Some(prepared_body) = prepared_body {
                                    object_writer.write::<_, _, 4>(&prepared_body.offset());
                                }
                                if let ::core::option::Option::Some(prepared_body) = prepared_body {
                                    object_writer.write::<_, _, 1>(&prepared_body.tag());
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<EventEntry>> for EventEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EventEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<EventEntry>> for EventEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EventEntry>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<EventEntry> for EventEntry {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EventEntry> {
                        EventEntry::create(builder, self.seq, self.round, &self.body)
                    }
                }

                /// Builder for serializing an instance of the [EventEntry] type.
                ///
                /// Can be created using the [EventEntry::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct EventEntryBuilder<State>(State);

                impl EventEntryBuilder<()> {
                    /// Setter for the [`seq` field](EventEntry#structfield.seq).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn seq<T0>(self, value: T0) -> EventEntryBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u64, u64>,
                    {
                        EventEntryBuilder((value,))
                    }

                    /// Sets the [`seq` field](EventEntry#structfield.seq) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn seq_as_default(self) -> EventEntryBuilder<(::planus::DefaultValue,)> {
                        self.seq(::planus::DefaultValue)
                    }
                }

                impl<T0> EventEntryBuilder<(T0,)> {
                    /// Setter for the [`round` field](EventEntry#structfield.round).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn round<T1>(self, value: T1) -> EventEntryBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0,) = self.0;
                        EventEntryBuilder((v0, value))
                    }

                    /// Sets the [`round` field](EventEntry#structfield.round) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn round_as_default(
                        self,
                    ) -> EventEntryBuilder<(T0, ::planus::DefaultValue)> {
                        self.round(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> EventEntryBuilder<(T0, T1)> {
                    /// Setter for the [`body` field](EventEntry#structfield.body).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn body<T2>(self, value: T2) -> EventEntryBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsOptionalUnion<self::EventBody>,
                    {
                        let (v0, v1) = self.0;
                        EventEntryBuilder((v0, v1, value))
                    }

                    /// Sets the [`body` field](EventEntry#structfield.body) to null.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn body_as_null(self) -> EventEntryBuilder<(T0, T1, ())> {
                        self.body(())
                    }
                }

                impl<T0, T1, T2> EventEntryBuilder<(T0, T1, T2)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [EventEntry].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EventEntry>
                    where
                        Self: ::planus::WriteAsOffset<EventEntry>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u64, u64>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsOptionalUnion<self::EventBody>,
                > ::planus::WriteAs<::planus::Offset<EventEntry>>
                    for EventEntryBuilder<(T0, T1, T2)>
                {
                    type Prepared = ::planus::Offset<EventEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EventEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u64, u64>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsOptionalUnion<self::EventBody>,
                > ::planus::WriteAsOptional<::planus::Offset<EventEntry>>
                    for EventEntryBuilder<(T0, T1, T2)>
                {
                    type Prepared = ::planus::Offset<EventEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<EventEntry>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u64, u64>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsOptionalUnion<self::EventBody>,
                > ::planus::WriteAsOffset<EventEntry> for EventEntryBuilder<(T0, T1, T2)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<EventEntry> {
                        let (v0, v1, v2) = &self.0;
                        EventEntry::create(builder, v0, v1, v2)
                    }
                }

                /// Reference to a deserialized [EventEntry].
                #[derive(Copy, Clone)]
                pub struct EventEntryRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

                impl<'a> EventEntryRef<'a> {
                    /// Getter for the [`seq` field](EventEntry#structfield.seq).
                    #[inline]
                    pub fn seq(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0.access(0, "EventEntry", "seq")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`round` field](EventEntry#structfield.round).
                    #[inline]
                    pub fn round(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "EventEntry", "round")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`body` field](EventEntry#structfield.body).
                    #[inline]
                    pub fn body(
                        &self,
                    ) -> ::planus::Result<::core::option::Option<self::EventBodyRef<'a>>>
                    {
                        self.0.access_union(2, "EventEntry", "body")
                    }
                }

                impl<'a> ::core::fmt::Debug for EventEntryRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("EventEntryRef");
                        f.field("seq", &self.seq());
                        f.field("round", &self.round());
                        if let ::core::option::Option::Some(field_body) = self.body().transpose() {
                            f.field("body", &field_body);
                        }
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<EventEntryRef<'a>> for EventEntry {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: EventEntryRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            seq: ::core::convert::TryInto::try_into(value.seq()?)?,
                            round: ::core::convert::TryInto::try_into(value.round()?)?,
                            body: if let ::core::option::Option::Some(body) = value.body()? {
                                ::core::option::Option::Some(::core::convert::TryInto::try_into(
                                    body,
                                )?)
                            } else {
                                ::core::option::Option::None
                            },
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for EventEntryRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for EventEntryRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[EventEntryRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<EventEntry>> for EventEntry {
                    type Value = ::planus::Offset<EventEntry>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<EventEntry>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for EventEntryRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[EventEntryRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `PortalEntry` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `PortalEntry` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:71`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct PortalEntry {
                    /// The field `id` in the table `PortalEntry`
                    pub id: u32,
                    /// The field `x` in the table `PortalEntry`
                    pub x: i32,
                    /// The field `y` in the table `PortalEntry`
                    pub y: i32,
                    /// The field `owner` in the table `PortalEntry`
                    pub owner: u8,
                    /// The field `hp` in the table `PortalEntry`
                    pub hp: u32,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for PortalEntry {
                    fn default() -> Self {
                        Self {
                            id: 0,
                            x: 0,
                            y: 0,
                            owner: 0,
                            hp: 0,
                        }
                    }
                }

                impl PortalEntry {
                    /// Creates a [PortalEntryBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> PortalEntryBuilder<()> {
                        PortalEntryBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_id: impl ::planus::WriteAsDefault<u32, u32>,
                        field_x: impl ::planus::WriteAsDefault<i32, i32>,
                        field_y: impl ::planus::WriteAsDefault<i32, i32>,
                        field_owner: impl ::planus::WriteAsDefault<u8, u8>,
                        field_hp: impl ::planus::WriteAsDefault<u32, u32>,
                    ) -> ::planus::Offset<Self> {
                        let prepared_id = field_id.prepare(builder, &0);
                        let prepared_x = field_x.prepare(builder, &0);
                        let prepared_y = field_y.prepare(builder, &0);
                        let prepared_owner = field_owner.prepare(builder, &0);
                        let prepared_hp = field_hp.prepare(builder, &0);

                        let mut table_writer: ::planus::table_writer::TableWriter<14> =
                            ::core::default::Default::default();
                        if prepared_id.is_some() {
                            table_writer.write_entry::<u32>(0);
                        }
                        if prepared_x.is_some() {
                            table_writer.write_entry::<i32>(1);
                        }
                        if prepared_y.is_some() {
                            table_writer.write_entry::<i32>(2);
                        }
                        if prepared_hp.is_some() {
                            table_writer.write_entry::<u32>(4);
                        }
                        if prepared_owner.is_some() {
                            table_writer.write_entry::<u8>(3);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_id) = prepared_id {
                                    object_writer.write::<_, _, 4>(&prepared_id);
                                }
                                if let ::core::option::Option::Some(prepared_x) = prepared_x {
                                    object_writer.write::<_, _, 4>(&prepared_x);
                                }
                                if let ::core::option::Option::Some(prepared_y) = prepared_y {
                                    object_writer.write::<_, _, 4>(&prepared_y);
                                }
                                if let ::core::option::Option::Some(prepared_hp) = prepared_hp {
                                    object_writer.write::<_, _, 4>(&prepared_hp);
                                }
                                if let ::core::option::Option::Some(prepared_owner) = prepared_owner
                                {
                                    object_writer.write::<_, _, 1>(&prepared_owner);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<PortalEntry>> for PortalEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PortalEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<PortalEntry>> for PortalEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<PortalEntry>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<PortalEntry> for PortalEntry {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PortalEntry> {
                        PortalEntry::create(builder, self.id, self.x, self.y, self.owner, self.hp)
                    }
                }

                /// Builder for serializing an instance of the [PortalEntry] type.
                ///
                /// Can be created using the [PortalEntry::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct PortalEntryBuilder<State>(State);

                impl PortalEntryBuilder<()> {
                    /// Setter for the [`id` field](PortalEntry#structfield.id).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn id<T0>(self, value: T0) -> PortalEntryBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u32, u32>,
                    {
                        PortalEntryBuilder((value,))
                    }

                    /// Sets the [`id` field](PortalEntry#structfield.id) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn id_as_default(self) -> PortalEntryBuilder<(::planus::DefaultValue,)> {
                        self.id(::planus::DefaultValue)
                    }
                }

                impl<T0> PortalEntryBuilder<(T0,)> {
                    /// Setter for the [`x` field](PortalEntry#structfield.x).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn x<T1>(self, value: T1) -> PortalEntryBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<i32, i32>,
                    {
                        let (v0,) = self.0;
                        PortalEntryBuilder((v0, value))
                    }

                    /// Sets the [`x` field](PortalEntry#structfield.x) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn x_as_default(self) -> PortalEntryBuilder<(T0, ::planus::DefaultValue)> {
                        self.x(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> PortalEntryBuilder<(T0, T1)> {
                    /// Setter for the [`y` field](PortalEntry#structfield.y).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn y<T2>(self, value: T2) -> PortalEntryBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<i32, i32>,
                    {
                        let (v0, v1) = self.0;
                        PortalEntryBuilder((v0, v1, value))
                    }

                    /// Sets the [`y` field](PortalEntry#structfield.y) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn y_as_default(
                        self,
                    ) -> PortalEntryBuilder<(T0, T1, ::planus::DefaultValue)> {
                        self.y(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> PortalEntryBuilder<(T0, T1, T2)> {
                    /// Setter for the [`owner` field](PortalEntry#structfield.owner).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn owner<T3>(self, value: T3) -> PortalEntryBuilder<(T0, T1, T2, T3)>
                    where
                        T3: ::planus::WriteAsDefault<u8, u8>,
                    {
                        let (v0, v1, v2) = self.0;
                        PortalEntryBuilder((v0, v1, v2, value))
                    }

                    /// Sets the [`owner` field](PortalEntry#structfield.owner) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn owner_as_default(
                        self,
                    ) -> PortalEntryBuilder<(T0, T1, T2, ::planus::DefaultValue)>
                    {
                        self.owner(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3> PortalEntryBuilder<(T0, T1, T2, T3)> {
                    /// Setter for the [`hp` field](PortalEntry#structfield.hp).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn hp<T4>(self, value: T4) -> PortalEntryBuilder<(T0, T1, T2, T3, T4)>
                    where
                        T4: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1, v2, v3) = self.0;
                        PortalEntryBuilder((v0, v1, v2, v3, value))
                    }

                    /// Sets the [`hp` field](PortalEntry#structfield.hp) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn hp_as_default(
                        self,
                    ) -> PortalEntryBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)>
                    {
                        self.hp(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4> PortalEntryBuilder<(T0, T1, T2, T3, T4)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [PortalEntry].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PortalEntry>
                    where
                        Self: ::planus::WriteAsOffset<PortalEntry>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<i32, i32>,
                    T2: ::planus::WriteAsDefault<i32, i32>,
                    T3: ::planus::WriteAsDefault<u8, u8>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAs<::planus::Offset<PortalEntry>>
                    for PortalEntryBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<PortalEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PortalEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<i32, i32>,
                    T2: ::planus::WriteAsDefault<i32, i32>,
                    T3: ::planus::WriteAsDefault<u8, u8>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOptional<::planus::Offset<PortalEntry>>
                    for PortalEntryBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<PortalEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<PortalEntry>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<i32, i32>,
                    T2: ::planus::WriteAsDefault<i32, i32>,
                    T3: ::planus::WriteAsDefault<u8, u8>,
                    T4: ::planus::WriteAsDefault<u32, u32>,
                > ::planus::WriteAsOffset<PortalEntry>
                    for PortalEntryBuilder<(T0, T1, T2, T3, T4)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PortalEntry> {
                        let (v0, v1, v2, v3, v4) = &self.0;
                        PortalEntry::create(builder, v0, v1, v2, v3, v4)
                    }
                }

                /// Reference to a deserialized [PortalEntry].
                #[derive(Copy, Clone)]
                pub struct PortalEntryRef<'a>(
                    #[allow(dead_code)] ::planus::table_reader::Table<'a>,
                );

                impl<'a> PortalEntryRef<'a> {
                    /// Getter for the [`id` field](PortalEntry#structfield.id).
                    #[inline]
                    pub fn id(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(0, "PortalEntry", "id")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`x` field](PortalEntry#structfield.x).
                    #[inline]
                    pub fn x(&self) -> ::planus::Result<i32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "PortalEntry", "x")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`y` field](PortalEntry#structfield.y).
                    #[inline]
                    pub fn y(&self) -> ::planus::Result<i32> {
                        ::core::result::Result::Ok(
                            self.0.access(2, "PortalEntry", "y")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`owner` field](PortalEntry#structfield.owner).
                    #[inline]
                    pub fn owner(&self) -> ::planus::Result<u8> {
                        ::core::result::Result::Ok(
                            self.0.access(3, "PortalEntry", "owner")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`hp` field](PortalEntry#structfield.hp).
                    #[inline]
                    pub fn hp(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(4, "PortalEntry", "hp")?.unwrap_or(0),
                        )
                    }
                }

                impl<'a> ::core::fmt::Debug for PortalEntryRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("PortalEntryRef");
                        f.field("id", &self.id());
                        f.field("x", &self.x());
                        f.field("y", &self.y());
                        f.field("owner", &self.owner());
                        f.field("hp", &self.hp());
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<PortalEntryRef<'a>> for PortalEntry {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: PortalEntryRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            id: ::core::convert::TryInto::try_into(value.id()?)?,
                            x: ::core::convert::TryInto::try_into(value.x()?)?,
                            y: ::core::convert::TryInto::try_into(value.y()?)?,
                            owner: ::core::convert::TryInto::try_into(value.owner()?)?,
                            hp: ::core::convert::TryInto::try_into(value.hp()?)?,
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for PortalEntryRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for PortalEntryRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[PortalEntryRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<PortalEntry>> for PortalEntry {
                    type Value = ::planus::Offset<PortalEntry>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<PortalEntry>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for PortalEntryRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[PortalEntryRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `LinkEntry` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `LinkEntry` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:79`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct LinkEntry {
                    /// The field `a` in the table `LinkEntry`
                    pub a: u32,
                    /// The field `b` in the table `LinkEntry`
                    pub b: u32,
                    /// The field `faction` in the table `LinkEntry`
                    pub faction: u8,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for LinkEntry {
                    fn default() -> Self {
                        Self {
                            a: 0,
                            b: 0,
                            faction: 0,
                        }
                    }
                }

                impl LinkEntry {
                    /// Creates a [LinkEntryBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> LinkEntryBuilder<()> {
                        LinkEntryBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_a: impl ::planus::WriteAsDefault<u32, u32>,
                        field_b: impl ::planus::WriteAsDefault<u32, u32>,
                        field_faction: impl ::planus::WriteAsDefault<u8, u8>,
                    ) -> ::planus::Offset<Self> {
                        let prepared_a = field_a.prepare(builder, &0);
                        let prepared_b = field_b.prepare(builder, &0);
                        let prepared_faction = field_faction.prepare(builder, &0);

                        let mut table_writer: ::planus::table_writer::TableWriter<10> =
                            ::core::default::Default::default();
                        if prepared_a.is_some() {
                            table_writer.write_entry::<u32>(0);
                        }
                        if prepared_b.is_some() {
                            table_writer.write_entry::<u32>(1);
                        }
                        if prepared_faction.is_some() {
                            table_writer.write_entry::<u8>(2);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_a) = prepared_a {
                                    object_writer.write::<_, _, 4>(&prepared_a);
                                }
                                if let ::core::option::Option::Some(prepared_b) = prepared_b {
                                    object_writer.write::<_, _, 4>(&prepared_b);
                                }
                                if let ::core::option::Option::Some(prepared_faction) =
                                    prepared_faction
                                {
                                    object_writer.write::<_, _, 1>(&prepared_faction);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<LinkEntry>> for LinkEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<LinkEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<LinkEntry>> for LinkEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<LinkEntry>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<LinkEntry> for LinkEntry {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<LinkEntry> {
                        LinkEntry::create(builder, self.a, self.b, self.faction)
                    }
                }

                /// Builder for serializing an instance of the [LinkEntry] type.
                ///
                /// Can be created using the [LinkEntry::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct LinkEntryBuilder<State>(State);

                impl LinkEntryBuilder<()> {
                    /// Setter for the [`a` field](LinkEntry#structfield.a).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn a<T0>(self, value: T0) -> LinkEntryBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u32, u32>,
                    {
                        LinkEntryBuilder((value,))
                    }

                    /// Sets the [`a` field](LinkEntry#structfield.a) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn a_as_default(self) -> LinkEntryBuilder<(::planus::DefaultValue,)> {
                        self.a(::planus::DefaultValue)
                    }
                }

                impl<T0> LinkEntryBuilder<(T0,)> {
                    /// Setter for the [`b` field](LinkEntry#structfield.b).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn b<T1>(self, value: T1) -> LinkEntryBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0,) = self.0;
                        LinkEntryBuilder((v0, value))
                    }

                    /// Sets the [`b` field](LinkEntry#structfield.b) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn b_as_default(self) -> LinkEntryBuilder<(T0, ::planus::DefaultValue)> {
                        self.b(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> LinkEntryBuilder<(T0, T1)> {
                    /// Setter for the [`faction` field](LinkEntry#structfield.faction).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction<T2>(self, value: T2) -> LinkEntryBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<u8, u8>,
                    {
                        let (v0, v1) = self.0;
                        LinkEntryBuilder((v0, v1, value))
                    }

                    /// Sets the [`faction` field](LinkEntry#structfield.faction) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction_as_default(
                        self,
                    ) -> LinkEntryBuilder<(T0, T1, ::planus::DefaultValue)> {
                        self.faction(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> LinkEntryBuilder<(T0, T1, T2)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [LinkEntry].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<LinkEntry>
                    where
                        Self: ::planus::WriteAsOffset<LinkEntry>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u8, u8>,
                > ::planus::WriteAs<::planus::Offset<LinkEntry>>
                    for LinkEntryBuilder<(T0, T1, T2)>
                {
                    type Prepared = ::planus::Offset<LinkEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<LinkEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u8, u8>,
                > ::planus::WriteAsOptional<::planus::Offset<LinkEntry>>
                    for LinkEntryBuilder<(T0, T1, T2)>
                {
                    type Prepared = ::planus::Offset<LinkEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<LinkEntry>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u8, u8>,
                > ::planus::WriteAsOffset<LinkEntry> for LinkEntryBuilder<(T0, T1, T2)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<LinkEntry> {
                        let (v0, v1, v2) = &self.0;
                        LinkEntry::create(builder, v0, v1, v2)
                    }
                }

                /// Reference to a deserialized [LinkEntry].
                #[derive(Copy, Clone)]
                pub struct LinkEntryRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

                impl<'a> LinkEntryRef<'a> {
                    /// Getter for the [`a` field](LinkEntry#structfield.a).
                    #[inline]
                    pub fn a(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(self.0.access(0, "LinkEntry", "a")?.unwrap_or(0))
                    }

                    /// Getter for the [`b` field](LinkEntry#structfield.b).
                    #[inline]
                    pub fn b(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(self.0.access(1, "LinkEntry", "b")?.unwrap_or(0))
                    }

                    /// Getter for the [`faction` field](LinkEntry#structfield.faction).
                    #[inline]
                    pub fn faction(&self) -> ::planus::Result<u8> {
                        ::core::result::Result::Ok(
                            self.0.access(2, "LinkEntry", "faction")?.unwrap_or(0),
                        )
                    }
                }

                impl<'a> ::core::fmt::Debug for LinkEntryRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("LinkEntryRef");
                        f.field("a", &self.a());
                        f.field("b", &self.b());
                        f.field("faction", &self.faction());
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<LinkEntryRef<'a>> for LinkEntry {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: LinkEntryRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            a: ::core::convert::TryInto::try_into(value.a()?)?,
                            b: ::core::convert::TryInto::try_into(value.b()?)?,
                            faction: ::core::convert::TryInto::try_into(value.faction()?)?,
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for LinkEntryRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for LinkEntryRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[LinkEntryRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<LinkEntry>> for LinkEntry {
                    type Value = ::planus::Offset<LinkEntry>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<LinkEntry>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for LinkEntryRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[LinkEntryRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `FieldEntry` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `FieldEntry` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:85`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct FieldEntry {
                    /// The field `a` in the table `FieldEntry`
                    pub a: u32,
                    /// The field `b` in the table `FieldEntry`
                    pub b: u32,
                    /// The field `c` in the table `FieldEntry`
                    pub c: u32,
                    /// The field `faction` in the table `FieldEntry`
                    pub faction: u8,
                    /// The field `area2` in the table `FieldEntry`
                    pub area2: u64,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for FieldEntry {
                    fn default() -> Self {
                        Self {
                            a: 0,
                            b: 0,
                            c: 0,
                            faction: 0,
                            area2: 0,
                        }
                    }
                }

                impl FieldEntry {
                    /// Creates a [FieldEntryBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> FieldEntryBuilder<()> {
                        FieldEntryBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_a: impl ::planus::WriteAsDefault<u32, u32>,
                        field_b: impl ::planus::WriteAsDefault<u32, u32>,
                        field_c: impl ::planus::WriteAsDefault<u32, u32>,
                        field_faction: impl ::planus::WriteAsDefault<u8, u8>,
                        field_area2: impl ::planus::WriteAsDefault<u64, u64>,
                    ) -> ::planus::Offset<Self> {
                        let prepared_a = field_a.prepare(builder, &0);
                        let prepared_b = field_b.prepare(builder, &0);
                        let prepared_c = field_c.prepare(builder, &0);
                        let prepared_faction = field_faction.prepare(builder, &0);
                        let prepared_area2 = field_area2.prepare(builder, &0);

                        let mut table_writer: ::planus::table_writer::TableWriter<14> =
                            ::core::default::Default::default();
                        if prepared_area2.is_some() {
                            table_writer.write_entry::<u64>(4);
                        }
                        if prepared_a.is_some() {
                            table_writer.write_entry::<u32>(0);
                        }
                        if prepared_b.is_some() {
                            table_writer.write_entry::<u32>(1);
                        }
                        if prepared_c.is_some() {
                            table_writer.write_entry::<u32>(2);
                        }
                        if prepared_faction.is_some() {
                            table_writer.write_entry::<u8>(3);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_area2) = prepared_area2
                                {
                                    object_writer.write::<_, _, 8>(&prepared_area2);
                                }
                                if let ::core::option::Option::Some(prepared_a) = prepared_a {
                                    object_writer.write::<_, _, 4>(&prepared_a);
                                }
                                if let ::core::option::Option::Some(prepared_b) = prepared_b {
                                    object_writer.write::<_, _, 4>(&prepared_b);
                                }
                                if let ::core::option::Option::Some(prepared_c) = prepared_c {
                                    object_writer.write::<_, _, 4>(&prepared_c);
                                }
                                if let ::core::option::Option::Some(prepared_faction) =
                                    prepared_faction
                                {
                                    object_writer.write::<_, _, 1>(&prepared_faction);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<FieldEntry>> for FieldEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<FieldEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<FieldEntry>> for FieldEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<FieldEntry>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<FieldEntry> for FieldEntry {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<FieldEntry> {
                        FieldEntry::create(
                            builder,
                            self.a,
                            self.b,
                            self.c,
                            self.faction,
                            self.area2,
                        )
                    }
                }

                /// Builder for serializing an instance of the [FieldEntry] type.
                ///
                /// Can be created using the [FieldEntry::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct FieldEntryBuilder<State>(State);

                impl FieldEntryBuilder<()> {
                    /// Setter for the [`a` field](FieldEntry#structfield.a).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn a<T0>(self, value: T0) -> FieldEntryBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u32, u32>,
                    {
                        FieldEntryBuilder((value,))
                    }

                    /// Sets the [`a` field](FieldEntry#structfield.a) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn a_as_default(self) -> FieldEntryBuilder<(::planus::DefaultValue,)> {
                        self.a(::planus::DefaultValue)
                    }
                }

                impl<T0> FieldEntryBuilder<(T0,)> {
                    /// Setter for the [`b` field](FieldEntry#structfield.b).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn b<T1>(self, value: T1) -> FieldEntryBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0,) = self.0;
                        FieldEntryBuilder((v0, value))
                    }

                    /// Sets the [`b` field](FieldEntry#structfield.b) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn b_as_default(self) -> FieldEntryBuilder<(T0, ::planus::DefaultValue)> {
                        self.b(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> FieldEntryBuilder<(T0, T1)> {
                    /// Setter for the [`c` field](FieldEntry#structfield.c).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn c<T2>(self, value: T2) -> FieldEntryBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1) = self.0;
                        FieldEntryBuilder((v0, v1, value))
                    }

                    /// Sets the [`c` field](FieldEntry#structfield.c) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn c_as_default(
                        self,
                    ) -> FieldEntryBuilder<(T0, T1, ::planus::DefaultValue)> {
                        self.c(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> FieldEntryBuilder<(T0, T1, T2)> {
                    /// Setter for the [`faction` field](FieldEntry#structfield.faction).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction<T3>(self, value: T3) -> FieldEntryBuilder<(T0, T1, T2, T3)>
                    where
                        T3: ::planus::WriteAsDefault<u8, u8>,
                    {
                        let (v0, v1, v2) = self.0;
                        FieldEntryBuilder((v0, v1, v2, value))
                    }

                    /// Sets the [`faction` field](FieldEntry#structfield.faction) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn faction_as_default(
                        self,
                    ) -> FieldEntryBuilder<(T0, T1, T2, ::planus::DefaultValue)>
                    {
                        self.faction(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3> FieldEntryBuilder<(T0, T1, T2, T3)> {
                    /// Setter for the [`area2` field](FieldEntry#structfield.area2).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn area2<T4>(self, value: T4) -> FieldEntryBuilder<(T0, T1, T2, T3, T4)>
                    where
                        T4: ::planus::WriteAsDefault<u64, u64>,
                    {
                        let (v0, v1, v2, v3) = self.0;
                        FieldEntryBuilder((v0, v1, v2, v3, value))
                    }

                    /// Sets the [`area2` field](FieldEntry#structfield.area2) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn area2_as_default(
                        self,
                    ) -> FieldEntryBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)>
                    {
                        self.area2(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4> FieldEntryBuilder<(T0, T1, T2, T3, T4)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [FieldEntry].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<FieldEntry>
                    where
                        Self: ::planus::WriteAsOffset<FieldEntry>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u8, u8>,
                    T4: ::planus::WriteAsDefault<u64, u64>,
                > ::planus::WriteAs<::planus::Offset<FieldEntry>>
                    for FieldEntryBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<FieldEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<FieldEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u8, u8>,
                    T4: ::planus::WriteAsDefault<u64, u64>,
                > ::planus::WriteAsOptional<::planus::Offset<FieldEntry>>
                    for FieldEntryBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<FieldEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<FieldEntry>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u32, u32>,
                    T3: ::planus::WriteAsDefault<u8, u8>,
                    T4: ::planus::WriteAsDefault<u64, u64>,
                > ::planus::WriteAsOffset<FieldEntry> for FieldEntryBuilder<(T0, T1, T2, T3, T4)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<FieldEntry> {
                        let (v0, v1, v2, v3, v4) = &self.0;
                        FieldEntry::create(builder, v0, v1, v2, v3, v4)
                    }
                }

                /// Reference to a deserialized [FieldEntry].
                #[derive(Copy, Clone)]
                pub struct FieldEntryRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

                impl<'a> FieldEntryRef<'a> {
                    /// Getter for the [`a` field](FieldEntry#structfield.a).
                    #[inline]
                    pub fn a(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(0, "FieldEntry", "a")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`b` field](FieldEntry#structfield.b).
                    #[inline]
                    pub fn b(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "FieldEntry", "b")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`c` field](FieldEntry#structfield.c).
                    #[inline]
                    pub fn c(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(2, "FieldEntry", "c")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`faction` field](FieldEntry#structfield.faction).
                    #[inline]
                    pub fn faction(&self) -> ::planus::Result<u8> {
                        ::core::result::Result::Ok(
                            self.0.access(3, "FieldEntry", "faction")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`area2` field](FieldEntry#structfield.area2).
                    #[inline]
                    pub fn area2(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0.access(4, "FieldEntry", "area2")?.unwrap_or(0),
                        )
                    }
                }

                impl<'a> ::core::fmt::Debug for FieldEntryRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("FieldEntryRef");
                        f.field("a", &self.a());
                        f.field("b", &self.b());
                        f.field("c", &self.c());
                        f.field("faction", &self.faction());
                        f.field("area2", &self.area2());
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<FieldEntryRef<'a>> for FieldEntry {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: FieldEntryRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            a: ::core::convert::TryInto::try_into(value.a()?)?,
                            b: ::core::convert::TryInto::try_into(value.b()?)?,
                            c: ::core::convert::TryInto::try_into(value.c()?)?,
                            faction: ::core::convert::TryInto::try_into(value.faction()?)?,
                            area2: ::core::convert::TryInto::try_into(value.area2()?)?,
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for FieldEntryRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for FieldEntryRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[FieldEntryRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<FieldEntry>> for FieldEntry {
                    type Value = ::planus::Offset<FieldEntry>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<FieldEntry>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for FieldEntryRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[FieldEntryRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `PublicSnapshot` in the namespace `ingress.snapshot.public`
                ///
                /// Generated from these locations:
                /// * Table `PublicSnapshot` in the file `crates/arena-protocol/schemas/public_snapshot.fbs:93`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct PublicSnapshot {
                    /// The field `schema_version` in the table `PublicSnapshot`
                    pub schema_version: u32,
                    /// The field `current_round` in the table `PublicSnapshot`
                    pub current_round: u32,
                    /// The field `snapshot_version` in the table `PublicSnapshot`
                    pub snapshot_version: u64,
                    /// The field `round_start_ms` in the table `PublicSnapshot`
                    pub round_start_ms: u64,
                    /// The field `deadline_ms` in the table `PublicSnapshot`
                    pub deadline_ms: u64,
                    /// The field `score_enl_hi` in the table `PublicSnapshot`
                    pub score_enl_hi: u64,
                    /// The field `score_enl_lo` in the table `PublicSnapshot`
                    pub score_enl_lo: u64,
                    /// The field `score_res_hi` in the table `PublicSnapshot`
                    pub score_res_hi: u64,
                    /// The field `score_res_lo` in the table `PublicSnapshot`
                    pub score_res_lo: u64,
                    /// The field `portals` in the table `PublicSnapshot`
                    pub portals:
                        ::core::option::Option<::planus::alloc::vec::Vec<self::PortalEntry>>,
                    /// The field `links` in the table `PublicSnapshot`
                    pub links: ::core::option::Option<::planus::alloc::vec::Vec<self::LinkEntry>>,
                    /// The field `fields` in the table `PublicSnapshot`
                    pub fields: ::core::option::Option<::planus::alloc::vec::Vec<self::FieldEntry>>,
                    /// The field `events` in the table `PublicSnapshot`
                    pub events: ::core::option::Option<::planus::alloc::vec::Vec<self::EventEntry>>,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for PublicSnapshot {
                    fn default() -> Self {
                        Self {
                            schema_version: 1,
                            current_round: 0,
                            snapshot_version: 0,
                            round_start_ms: 0,
                            deadline_ms: 0,
                            score_enl_hi: 0,
                            score_enl_lo: 0,
                            score_res_hi: 0,
                            score_res_lo: 0,
                            portals: ::core::default::Default::default(),
                            links: ::core::default::Default::default(),
                            fields: ::core::default::Default::default(),
                            events: ::core::default::Default::default(),
                        }
                    }
                }

                impl PublicSnapshot {
                    /// Creates a [PublicSnapshotBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> PublicSnapshotBuilder<()> {
                        PublicSnapshotBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_schema_version: impl ::planus::WriteAsDefault<u32, u32>,
                        field_current_round: impl ::planus::WriteAsDefault<u32, u32>,
                        field_snapshot_version: impl ::planus::WriteAsDefault<u64, u64>,
                        field_round_start_ms: impl ::planus::WriteAsDefault<u64, u64>,
                        field_deadline_ms: impl ::planus::WriteAsDefault<u64, u64>,
                        field_score_enl_hi: impl ::planus::WriteAsDefault<u64, u64>,
                        field_score_enl_lo: impl ::planus::WriteAsDefault<u64, u64>,
                        field_score_res_hi: impl ::planus::WriteAsDefault<u64, u64>,
                        field_score_res_lo: impl ::planus::WriteAsDefault<u64, u64>,
                        field_portals: impl ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::PortalEntry>]>,
                        >,
                        field_links: impl ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::LinkEntry>]>,
                        >,
                        field_fields: impl ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::FieldEntry>]>,
                        >,
                        field_events: impl ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::EventEntry>]>,
                        >,
                    ) -> ::planus::Offset<Self> {
                        let prepared_schema_version = field_schema_version.prepare(builder, &1);
                        let prepared_current_round = field_current_round.prepare(builder, &0);
                        let prepared_snapshot_version = field_snapshot_version.prepare(builder, &0);
                        let prepared_round_start_ms = field_round_start_ms.prepare(builder, &0);
                        let prepared_deadline_ms = field_deadline_ms.prepare(builder, &0);
                        let prepared_score_enl_hi = field_score_enl_hi.prepare(builder, &0);
                        let prepared_score_enl_lo = field_score_enl_lo.prepare(builder, &0);
                        let prepared_score_res_hi = field_score_res_hi.prepare(builder, &0);
                        let prepared_score_res_lo = field_score_res_lo.prepare(builder, &0);
                        let prepared_portals = field_portals.prepare(builder);
                        let prepared_links = field_links.prepare(builder);
                        let prepared_fields = field_fields.prepare(builder);
                        let prepared_events = field_events.prepare(builder);

                        let mut table_writer: ::planus::table_writer::TableWriter<30> =
                            ::core::default::Default::default();
                        if prepared_snapshot_version.is_some() {
                            table_writer.write_entry::<u64>(2);
                        }
                        if prepared_round_start_ms.is_some() {
                            table_writer.write_entry::<u64>(3);
                        }
                        if prepared_deadline_ms.is_some() {
                            table_writer.write_entry::<u64>(4);
                        }
                        if prepared_score_enl_hi.is_some() {
                            table_writer.write_entry::<u64>(5);
                        }
                        if prepared_score_enl_lo.is_some() {
                            table_writer.write_entry::<u64>(6);
                        }
                        if prepared_score_res_hi.is_some() {
                            table_writer.write_entry::<u64>(7);
                        }
                        if prepared_score_res_lo.is_some() {
                            table_writer.write_entry::<u64>(8);
                        }
                        if prepared_schema_version.is_some() {
                            table_writer.write_entry::<u32>(0);
                        }
                        if prepared_current_round.is_some() {
                            table_writer.write_entry::<u32>(1);
                        }
                        if prepared_portals.is_some() {
                            table_writer.write_entry::<::planus::Offset<[::planus::Offset<self::PortalEntry>]>>(9);
                        }
                        if prepared_links.is_some() {
                            table_writer.write_entry::<::planus::Offset<[::planus::Offset<self::LinkEntry>]>>(10);
                        }
                        if prepared_fields.is_some() {
                            table_writer.write_entry::<::planus::Offset<[::planus::Offset<self::FieldEntry>]>>(11);
                        }
                        if prepared_events.is_some() {
                            table_writer.write_entry::<::planus::Offset<[::planus::Offset<self::EventEntry>]>>(12);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_snapshot_version) =
                                    prepared_snapshot_version
                                {
                                    object_writer.write::<_, _, 8>(&prepared_snapshot_version);
                                }
                                if let ::core::option::Option::Some(prepared_round_start_ms) =
                                    prepared_round_start_ms
                                {
                                    object_writer.write::<_, _, 8>(&prepared_round_start_ms);
                                }
                                if let ::core::option::Option::Some(prepared_deadline_ms) =
                                    prepared_deadline_ms
                                {
                                    object_writer.write::<_, _, 8>(&prepared_deadline_ms);
                                }
                                if let ::core::option::Option::Some(prepared_score_enl_hi) =
                                    prepared_score_enl_hi
                                {
                                    object_writer.write::<_, _, 8>(&prepared_score_enl_hi);
                                }
                                if let ::core::option::Option::Some(prepared_score_enl_lo) =
                                    prepared_score_enl_lo
                                {
                                    object_writer.write::<_, _, 8>(&prepared_score_enl_lo);
                                }
                                if let ::core::option::Option::Some(prepared_score_res_hi) =
                                    prepared_score_res_hi
                                {
                                    object_writer.write::<_, _, 8>(&prepared_score_res_hi);
                                }
                                if let ::core::option::Option::Some(prepared_score_res_lo) =
                                    prepared_score_res_lo
                                {
                                    object_writer.write::<_, _, 8>(&prepared_score_res_lo);
                                }
                                if let ::core::option::Option::Some(prepared_schema_version) =
                                    prepared_schema_version
                                {
                                    object_writer.write::<_, _, 4>(&prepared_schema_version);
                                }
                                if let ::core::option::Option::Some(prepared_current_round) =
                                    prepared_current_round
                                {
                                    object_writer.write::<_, _, 4>(&prepared_current_round);
                                }
                                if let ::core::option::Option::Some(prepared_portals) =
                                    prepared_portals
                                {
                                    object_writer.write::<_, _, 4>(&prepared_portals);
                                }
                                if let ::core::option::Option::Some(prepared_links) = prepared_links
                                {
                                    object_writer.write::<_, _, 4>(&prepared_links);
                                }
                                if let ::core::option::Option::Some(prepared_fields) =
                                    prepared_fields
                                {
                                    object_writer.write::<_, _, 4>(&prepared_fields);
                                }
                                if let ::core::option::Option::Some(prepared_events) =
                                    prepared_events
                                {
                                    object_writer.write::<_, _, 4>(&prepared_events);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<PublicSnapshot>> for PublicSnapshot {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PublicSnapshot> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<PublicSnapshot>> for PublicSnapshot {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<PublicSnapshot>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<PublicSnapshot> for PublicSnapshot {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PublicSnapshot> {
                        PublicSnapshot::create(
                            builder,
                            self.schema_version,
                            self.current_round,
                            self.snapshot_version,
                            self.round_start_ms,
                            self.deadline_ms,
                            self.score_enl_hi,
                            self.score_enl_lo,
                            self.score_res_hi,
                            self.score_res_lo,
                            &self.portals,
                            &self.links,
                            &self.fields,
                            &self.events,
                        )
                    }
                }

                /// Builder for serializing an instance of the [PublicSnapshot] type.
                ///
                /// Can be created using the [PublicSnapshot::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct PublicSnapshotBuilder<State>(State);

                impl PublicSnapshotBuilder<()> {
                    /// Setter for the [`schema_version` field](PublicSnapshot#structfield.schema_version).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn schema_version<T0>(self, value: T0) -> PublicSnapshotBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u32, u32>,
                    {
                        PublicSnapshotBuilder((value,))
                    }

                    /// Sets the [`schema_version` field](PublicSnapshot#structfield.schema_version) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn schema_version_as_default(
                        self,
                    ) -> PublicSnapshotBuilder<(::planus::DefaultValue,)> {
                        self.schema_version(::planus::DefaultValue)
                    }
                }

                impl<T0> PublicSnapshotBuilder<(T0,)> {
                    /// Setter for the [`current_round` field](PublicSnapshot#structfield.current_round).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn current_round<T1>(self, value: T1) -> PublicSnapshotBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0,) = self.0;
                        PublicSnapshotBuilder((v0, value))
                    }

                    /// Sets the [`current_round` field](PublicSnapshot#structfield.current_round) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn current_round_as_default(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, ::planus::DefaultValue)> {
                        self.current_round(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> PublicSnapshotBuilder<(T0, T1)> {
                    /// Setter for the [`snapshot_version` field](PublicSnapshot#structfield.snapshot_version).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn snapshot_version<T2>(
                        self,
                        value: T2,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<u64, u64>,
                    {
                        let (v0, v1) = self.0;
                        PublicSnapshotBuilder((v0, v1, value))
                    }

                    /// Sets the [`snapshot_version` field](PublicSnapshot#structfield.snapshot_version) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn snapshot_version_as_default(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, ::planus::DefaultValue)>
                    {
                        self.snapshot_version(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> PublicSnapshotBuilder<(T0, T1, T2)> {
                    /// Setter for the [`round_start_ms` field](PublicSnapshot#structfield.round_start_ms).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn round_start_ms<T3>(
                        self,
                        value: T3,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3)>
                    where
                        T3: ::planus::WriteAsDefault<u64, u64>,
                    {
                        let (v0, v1, v2) = self.0;
                        PublicSnapshotBuilder((v0, v1, v2, value))
                    }

                    /// Sets the [`round_start_ms` field](PublicSnapshot#structfield.round_start_ms) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn round_start_ms_as_default(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, ::planus::DefaultValue)>
                    {
                        self.round_start_ms(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3> PublicSnapshotBuilder<(T0, T1, T2, T3)> {
                    /// Setter for the [`deadline_ms` field](PublicSnapshot#structfield.deadline_ms).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn deadline_ms<T4>(
                        self,
                        value: T4,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4)>
                    where
                        T4: ::planus::WriteAsDefault<u64, u64>,
                    {
                        let (v0, v1, v2, v3) = self.0;
                        PublicSnapshotBuilder((v0, v1, v2, v3, value))
                    }

                    /// Sets the [`deadline_ms` field](PublicSnapshot#structfield.deadline_ms) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn deadline_ms_as_default(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)>
                    {
                        self.deadline_ms(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4> PublicSnapshotBuilder<(T0, T1, T2, T3, T4)> {
                    /// Setter for the [`score_enl_hi` field](PublicSnapshot#structfield.score_enl_hi).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn score_enl_hi<T5>(
                        self,
                        value: T5,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5)>
                    where
                        T5: ::planus::WriteAsDefault<u64, u64>,
                    {
                        let (v0, v1, v2, v3, v4) = self.0;
                        PublicSnapshotBuilder((v0, v1, v2, v3, v4, value))
                    }

                    /// Sets the [`score_enl_hi` field](PublicSnapshot#structfield.score_enl_hi) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn score_enl_hi_as_default(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, ::planus::DefaultValue)>
                    {
                        self.score_enl_hi(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4, T5> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5)> {
                    /// Setter for the [`score_enl_lo` field](PublicSnapshot#structfield.score_enl_lo).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn score_enl_lo<T6>(
                        self,
                        value: T6,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6)>
                    where
                        T6: ::planus::WriteAsDefault<u64, u64>,
                    {
                        let (v0, v1, v2, v3, v4, v5) = self.0;
                        PublicSnapshotBuilder((v0, v1, v2, v3, v4, v5, value))
                    }

                    /// Sets the [`score_enl_lo` field](PublicSnapshot#structfield.score_enl_lo) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn score_enl_lo_as_default(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, ::planus::DefaultValue)>
                    {
                        self.score_enl_lo(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4, T5, T6> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6)> {
                    /// Setter for the [`score_res_hi` field](PublicSnapshot#structfield.score_res_hi).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn score_res_hi<T7>(
                        self,
                        value: T7,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)>
                    where
                        T7: ::planus::WriteAsDefault<u64, u64>,
                    {
                        let (v0, v1, v2, v3, v4, v5, v6) = self.0;
                        PublicSnapshotBuilder((v0, v1, v2, v3, v4, v5, v6, value))
                    }

                    /// Sets the [`score_res_hi` field](PublicSnapshot#structfield.score_res_hi) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn score_res_hi_as_default(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, ::planus::DefaultValue)>
                    {
                        self.score_res_hi(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4, T5, T6, T7> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)> {
                    /// Setter for the [`score_res_lo` field](PublicSnapshot#structfield.score_res_lo).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn score_res_lo<T8>(
                        self,
                        value: T8,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)>
                    where
                        T8: ::planus::WriteAsDefault<u64, u64>,
                    {
                        let (v0, v1, v2, v3, v4, v5, v6, v7) = self.0;
                        PublicSnapshotBuilder((v0, v1, v2, v3, v4, v5, v6, v7, value))
                    }

                    /// Sets the [`score_res_lo` field](PublicSnapshot#structfield.score_res_lo) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn score_res_lo_as_default(
                        self,
                    ) -> PublicSnapshotBuilder<(
                        T0,
                        T1,
                        T2,
                        T3,
                        T4,
                        T5,
                        T6,
                        T7,
                        ::planus::DefaultValue,
                    )> {
                        self.score_res_lo(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3, T4, T5, T6, T7, T8>
                    PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)>
                {
                    /// Setter for the [`portals` field](PublicSnapshot#structfield.portals).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn portals<T9>(
                        self,
                        value: T9,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
                    where
                        T9: ::planus::WriteAsOptional<
                                ::planus::Offset<[::planus::Offset<self::PortalEntry>]>,
                            >,
                    {
                        let (v0, v1, v2, v3, v4, v5, v6, v7, v8) = self.0;
                        PublicSnapshotBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, value))
                    }

                    /// Sets the [`portals` field](PublicSnapshot#structfield.portals) to null.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn portals_as_null(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, ())>
                    {
                        self.portals(())
                    }
                }

                impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9>
                    PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
                {
                    /// Setter for the [`links` field](PublicSnapshot#structfield.links).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn links<T10>(
                        self,
                        value: T10,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)>
                    where
                        T10: ::planus::WriteAsOptional<
                                ::planus::Offset<[::planus::Offset<self::LinkEntry>]>,
                            >,
                    {
                        let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9) = self.0;
                        PublicSnapshotBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, value))
                    }

                    /// Sets the [`links` field](PublicSnapshot#structfield.links) to null.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn links_as_null(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, ())>
                    {
                        self.links(())
                    }
                }

                impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10>
                    PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)>
                {
                    /// Setter for the [`fields` field](PublicSnapshot#structfield.fields).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn fields<T11>(
                        self,
                        value: T11,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)>
                    where
                        T11: ::planus::WriteAsOptional<
                                ::planus::Offset<[::planus::Offset<self::FieldEntry>]>,
                            >,
                    {
                        let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10) = self.0;
                        PublicSnapshotBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, value))
                    }

                    /// Sets the [`fields` field](PublicSnapshot#structfield.fields) to null.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn fields_as_null(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, ())>
                    {
                        self.fields(())
                    }
                }

                impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11>
                    PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)>
                {
                    /// Setter for the [`events` field](PublicSnapshot#structfield.events).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn events<T12>(
                        self,
                        value: T12,
                    ) -> PublicSnapshotBuilder<(
                        T0,
                        T1,
                        T2,
                        T3,
                        T4,
                        T5,
                        T6,
                        T7,
                        T8,
                        T9,
                        T10,
                        T11,
                        T12,
                    )>
                    where
                        T12: ::planus::WriteAsOptional<
                                ::planus::Offset<[::planus::Offset<self::EventEntry>]>,
                            >,
                    {
                        let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11) = self.0;
                        PublicSnapshotBuilder((
                            v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, value,
                        ))
                    }

                    /// Sets the [`events` field](PublicSnapshot#structfield.events) to null.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn events_as_null(
                        self,
                    ) -> PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, ())>
                    {
                        self.events(())
                    }
                }

                impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12>
                    PublicSnapshotBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
                {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [PublicSnapshot].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PublicSnapshot>
                    where
                        Self: ::planus::WriteAsOffset<PublicSnapshot>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u64, u64>,
                    T3: ::planus::WriteAsDefault<u64, u64>,
                    T4: ::planus::WriteAsDefault<u64, u64>,
                    T5: ::planus::WriteAsDefault<u64, u64>,
                    T6: ::planus::WriteAsDefault<u64, u64>,
                    T7: ::planus::WriteAsDefault<u64, u64>,
                    T8: ::planus::WriteAsDefault<u64, u64>,
                    T9: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::PortalEntry>]>,
                        >,
                    T10: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::LinkEntry>]>,
                        >,
                    T11: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::FieldEntry>]>,
                        >,
                    T12: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::EventEntry>]>,
                        >,
                > ::planus::WriteAs<::planus::Offset<PublicSnapshot>>
                    for PublicSnapshotBuilder<(
                        T0,
                        T1,
                        T2,
                        T3,
                        T4,
                        T5,
                        T6,
                        T7,
                        T8,
                        T9,
                        T10,
                        T11,
                        T12,
                    )>
                {
                    type Prepared = ::planus::Offset<PublicSnapshot>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PublicSnapshot> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u64, u64>,
                    T3: ::planus::WriteAsDefault<u64, u64>,
                    T4: ::planus::WriteAsDefault<u64, u64>,
                    T5: ::planus::WriteAsDefault<u64, u64>,
                    T6: ::planus::WriteAsDefault<u64, u64>,
                    T7: ::planus::WriteAsDefault<u64, u64>,
                    T8: ::planus::WriteAsDefault<u64, u64>,
                    T9: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::PortalEntry>]>,
                        >,
                    T10: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::LinkEntry>]>,
                        >,
                    T11: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::FieldEntry>]>,
                        >,
                    T12: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::EventEntry>]>,
                        >,
                > ::planus::WriteAsOptional<::planus::Offset<PublicSnapshot>>
                    for PublicSnapshotBuilder<(
                        T0,
                        T1,
                        T2,
                        T3,
                        T4,
                        T5,
                        T6,
                        T7,
                        T8,
                        T9,
                        T10,
                        T11,
                        T12,
                    )>
                {
                    type Prepared = ::planus::Offset<PublicSnapshot>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<PublicSnapshot>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u64, u64>,
                    T3: ::planus::WriteAsDefault<u64, u64>,
                    T4: ::planus::WriteAsDefault<u64, u64>,
                    T5: ::planus::WriteAsDefault<u64, u64>,
                    T6: ::planus::WriteAsDefault<u64, u64>,
                    T7: ::planus::WriteAsDefault<u64, u64>,
                    T8: ::planus::WriteAsDefault<u64, u64>,
                    T9: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::PortalEntry>]>,
                        >,
                    T10: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::LinkEntry>]>,
                        >,
                    T11: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::FieldEntry>]>,
                        >,
                    T12: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::EventEntry>]>,
                        >,
                > ::planus::WriteAsOffset<PublicSnapshot>
                    for PublicSnapshotBuilder<(
                        T0,
                        T1,
                        T2,
                        T3,
                        T4,
                        T5,
                        T6,
                        T7,
                        T8,
                        T9,
                        T10,
                        T11,
                        T12,
                    )>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PublicSnapshot> {
                        let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12) = &self.0;
                        PublicSnapshot::create(
                            builder, v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12,
                        )
                    }
                }

                /// Reference to a deserialized [PublicSnapshot].
                #[derive(Copy, Clone)]
                pub struct PublicSnapshotRef<'a>(
                    #[allow(dead_code)] ::planus::table_reader::Table<'a>,
                );

                impl<'a> PublicSnapshotRef<'a> {
                    /// Getter for the [`schema_version` field](PublicSnapshot#structfield.schema_version).
                    #[inline]
                    pub fn schema_version(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(0, "PublicSnapshot", "schema_version")?
                                .unwrap_or(1),
                        )
                    }

                    /// Getter for the [`current_round` field](PublicSnapshot#structfield.current_round).
                    #[inline]
                    pub fn current_round(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(1, "PublicSnapshot", "current_round")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`snapshot_version` field](PublicSnapshot#structfield.snapshot_version).
                    #[inline]
                    pub fn snapshot_version(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(2, "PublicSnapshot", "snapshot_version")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`round_start_ms` field](PublicSnapshot#structfield.round_start_ms).
                    #[inline]
                    pub fn round_start_ms(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(3, "PublicSnapshot", "round_start_ms")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`deadline_ms` field](PublicSnapshot#structfield.deadline_ms).
                    #[inline]
                    pub fn deadline_ms(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(4, "PublicSnapshot", "deadline_ms")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`score_enl_hi` field](PublicSnapshot#structfield.score_enl_hi).
                    #[inline]
                    pub fn score_enl_hi(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(5, "PublicSnapshot", "score_enl_hi")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`score_enl_lo` field](PublicSnapshot#structfield.score_enl_lo).
                    #[inline]
                    pub fn score_enl_lo(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(6, "PublicSnapshot", "score_enl_lo")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`score_res_hi` field](PublicSnapshot#structfield.score_res_hi).
                    #[inline]
                    pub fn score_res_hi(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(7, "PublicSnapshot", "score_res_hi")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`score_res_lo` field](PublicSnapshot#structfield.score_res_lo).
                    #[inline]
                    pub fn score_res_lo(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(8, "PublicSnapshot", "score_res_lo")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`portals` field](PublicSnapshot#structfield.portals).
                    #[inline]
                    pub fn portals(
                        &self,
                    ) -> ::planus::Result<
                        ::core::option::Option<
                            ::planus::Vector<'a, ::planus::Result<self::PortalEntryRef<'a>>>,
                        >,
                    > {
                        self.0.access(9, "PublicSnapshot", "portals")
                    }

                    /// Getter for the [`links` field](PublicSnapshot#structfield.links).
                    #[inline]
                    pub fn links(
                        &self,
                    ) -> ::planus::Result<
                        ::core::option::Option<
                            ::planus::Vector<'a, ::planus::Result<self::LinkEntryRef<'a>>>,
                        >,
                    > {
                        self.0.access(10, "PublicSnapshot", "links")
                    }

                    /// Getter for the [`fields` field](PublicSnapshot#structfield.fields).
                    #[inline]
                    pub fn fields(
                        &self,
                    ) -> ::planus::Result<
                        ::core::option::Option<
                            ::planus::Vector<'a, ::planus::Result<self::FieldEntryRef<'a>>>,
                        >,
                    > {
                        self.0.access(11, "PublicSnapshot", "fields")
                    }

                    /// Getter for the [`events` field](PublicSnapshot#structfield.events).
                    #[inline]
                    pub fn events(
                        &self,
                    ) -> ::planus::Result<
                        ::core::option::Option<
                            ::planus::Vector<'a, ::planus::Result<self::EventEntryRef<'a>>>,
                        >,
                    > {
                        self.0.access(12, "PublicSnapshot", "events")
                    }
                }

                impl<'a> ::core::fmt::Debug for PublicSnapshotRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("PublicSnapshotRef");
                        f.field("schema_version", &self.schema_version());
                        f.field("current_round", &self.current_round());
                        f.field("snapshot_version", &self.snapshot_version());
                        f.field("round_start_ms", &self.round_start_ms());
                        f.field("deadline_ms", &self.deadline_ms());
                        f.field("score_enl_hi", &self.score_enl_hi());
                        f.field("score_enl_lo", &self.score_enl_lo());
                        f.field("score_res_hi", &self.score_res_hi());
                        f.field("score_res_lo", &self.score_res_lo());
                        if let ::core::option::Option::Some(field_portals) =
                            self.portals().transpose()
                        {
                            f.field("portals", &field_portals);
                        }
                        if let ::core::option::Option::Some(field_links) = self.links().transpose()
                        {
                            f.field("links", &field_links);
                        }
                        if let ::core::option::Option::Some(field_fields) =
                            self.fields().transpose()
                        {
                            f.field("fields", &field_fields);
                        }
                        if let ::core::option::Option::Some(field_events) =
                            self.events().transpose()
                        {
                            f.field("events", &field_events);
                        }
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<PublicSnapshotRef<'a>> for PublicSnapshot {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: PublicSnapshotRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            schema_version: ::core::convert::TryInto::try_into(
                                value.schema_version()?,
                            )?,
                            current_round: ::core::convert::TryInto::try_into(
                                value.current_round()?,
                            )?,
                            snapshot_version: ::core::convert::TryInto::try_into(
                                value.snapshot_version()?,
                            )?,
                            round_start_ms: ::core::convert::TryInto::try_into(
                                value.round_start_ms()?,
                            )?,
                            deadline_ms: ::core::convert::TryInto::try_into(value.deadline_ms()?)?,
                            score_enl_hi: ::core::convert::TryInto::try_into(
                                value.score_enl_hi()?,
                            )?,
                            score_enl_lo: ::core::convert::TryInto::try_into(
                                value.score_enl_lo()?,
                            )?,
                            score_res_hi: ::core::convert::TryInto::try_into(
                                value.score_res_hi()?,
                            )?,
                            score_res_lo: ::core::convert::TryInto::try_into(
                                value.score_res_lo()?,
                            )?,
                            portals: if let ::core::option::Option::Some(portals) =
                                value.portals()?
                            {
                                ::core::option::Option::Some(portals.to_vec_result()?)
                            } else {
                                ::core::option::Option::None
                            },
                            links: if let ::core::option::Option::Some(links) = value.links()? {
                                ::core::option::Option::Some(links.to_vec_result()?)
                            } else {
                                ::core::option::Option::None
                            },
                            fields: if let ::core::option::Option::Some(fields) = value.fields()? {
                                ::core::option::Option::Some(fields.to_vec_result()?)
                            } else {
                                ::core::option::Option::None
                            },
                            events: if let ::core::option::Option::Some(events) = value.events()? {
                                ::core::option::Option::Some(events.to_vec_result()?)
                            } else {
                                ::core::option::Option::None
                            },
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for PublicSnapshotRef<'a> {
                    #[inline]
                    fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
                    }
                }

                impl<'a> ::planus::VectorReadInner<'a> for PublicSnapshotRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[PublicSnapshotRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<PublicSnapshot>> for PublicSnapshot {
                    type Value = ::planus::Offset<PublicSnapshot>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<PublicSnapshot>],
                        bytes: *mut ::core::mem::MaybeUninit<u8>,
                        buffer_position: u32,
                    ) {
                        let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                        for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                            ::planus::WriteAsPrimitive::write(
                                v,
                                ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                                buffer_position - (Self::STRIDE * i) as u32,
                            );
                        }
                    }
                }

                impl<'a> ::planus::ReadAsRoot<'a> for PublicSnapshotRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[PublicSnapshotRef]", "read_as_root", 0)
                        })
                    }
                }
            }
        }
    }
}
