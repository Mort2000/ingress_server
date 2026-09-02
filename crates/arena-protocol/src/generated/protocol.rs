pub use root::*;

const _: () = ::planus::check_version_compatibility("planus-1.3.0");

/// The root namespace
///
/// Generated from these locations:
/// * File `crates/arena-protocol/schemas/protocol.fbs`
#[no_implicit_prelude]
#[allow(clippy::needless_lifetimes)]
mod root {
    /// The namespace `ingress`
    ///
    /// Generated from these locations:
    /// * File `crates/arena-protocol/schemas/protocol.fbs`
    pub mod ingress {
        /// The namespace `ingress.protocol`
        ///
        /// Generated from these locations:
        /// * File `crates/arena-protocol/schemas/protocol.fbs`
        pub mod protocol {
            ///  Faction identity (D51: 0 = ENL, 1 = RES).
            ///
            /// Generated from these locations:
            /// * Enum `Faction` in the file `crates/arena-protocol/schemas/protocol.fbs:16`
            #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
            #[repr(u8)]
            pub enum Faction {
                /// The variant `Enl` in the enum `Faction`
                Enl = 0,

                /// The variant `Res` in the enum `Faction`
                Res = 1,
            }

            impl Faction {
                /// Array containing all valid variants of Faction
                pub const ENUM_VALUES: [Self; 2] = [Self::Enl, Self::Res];
            }

            impl ::core::convert::TryFrom<u8> for Faction {
                type Error = ::planus::errors::UnknownEnumTagKind;
                #[inline]
                fn try_from(
                    value: u8,
                ) -> ::core::result::Result<Self, ::planus::errors::UnknownEnumTagKind>
                {
                    #[allow(clippy::match_single_binding)]
                    match value {
                        0 => ::core::result::Result::Ok(Faction::Enl),
                        1 => ::core::result::Result::Ok(Faction::Res),

                        _ => ::core::result::Result::Err(::planus::errors::UnknownEnumTagKind {
                            tag: value as i128,
                        }),
                    }
                }
            }

            impl ::core::convert::From<Faction> for u8 {
                #[inline]
                fn from(value: Faction) -> Self {
                    value as u8
                }
            }

            /// # Safety
            /// The Planus compiler correctly calculates `ALIGNMENT` and `SIZE`.
            unsafe impl ::planus::Primitive for Faction {
                const ALIGNMENT: usize = 1;
                const SIZE: usize = 1;
            }

            impl ::planus::WriteAsPrimitive<Faction> for Faction {
                #[inline]
                fn write<const N: usize>(
                    &self,
                    cursor: ::planus::Cursor<'_, N>,
                    buffer_position: u32,
                ) {
                    (*self as u8).write(cursor, buffer_position);
                }
            }

            impl ::planus::WriteAs<Faction> for Faction {
                type Prepared = Self;

                #[inline]
                fn prepare(&self, _builder: &mut ::planus::Builder) -> Faction {
                    *self
                }
            }

            impl ::planus::WriteAsDefault<Faction, Faction> for Faction {
                type Prepared = Self;

                #[inline]
                fn prepare(
                    &self,
                    _builder: &mut ::planus::Builder,
                    default: &Faction,
                ) -> ::core::option::Option<Faction> {
                    if self == default {
                        ::core::option::Option::None
                    } else {
                        ::core::option::Option::Some(*self)
                    }
                }
            }

            impl ::planus::WriteAsOptional<Faction> for Faction {
                type Prepared = Self;

                #[inline]
                fn prepare(
                    &self,
                    _builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<Faction> {
                    ::core::option::Option::Some(*self)
                }
            }

            impl<'buf> ::planus::TableRead<'buf> for Faction {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'buf>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    let n: u8 = ::planus::TableRead::from_buffer(buffer, offset)?;
                    ::core::result::Result::Ok(::core::convert::TryInto::try_into(n)?)
                }
            }

            impl<'buf> ::planus::VectorReadInner<'buf> for Faction {
                type Error = ::planus::errors::UnknownEnumTag;
                const STRIDE: usize = 1;
                #[inline]
                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'buf>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::UnknownEnumTag>
                {
                    let value = unsafe { *buffer.buffer.get_unchecked(offset) };
                    let value: ::core::result::Result<Self, _> =
                        ::core::convert::TryInto::try_into(value);
                    value.map_err(|error_kind| {
                        error_kind.with_error_location(
                            "Faction",
                            "VectorRead::from_buffer",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<Faction> for Faction {
                const STRIDE: usize = 1;

                type Value = Self;

                #[inline]
                fn prepare(&self, _builder: &mut ::planus::Builder) -> Self {
                    *self
                }

                #[inline]
                unsafe fn write_values(
                    values: &[Self],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 1];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - i as u32,
                        );
                    }
                }
            }

            ///  The eleven P3-RPC messages.
            ///
            /// Generated from these locations:
            /// * Union `Message` in the file `crates/arena-protocol/schemas/protocol.fbs:19`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub enum Message {
                /// The variant of type `ClientHello` in the union `Message`
                ClientHello(::planus::alloc::boxed::Box<self::ClientHello>),

                /// The variant of type `ServerHello` in the union `Message`
                ServerHello(::planus::alloc::boxed::Box<self::ServerHello>),

                /// The variant of type `InitialSpawnStart` in the union `Message`
                InitialSpawnStart(::planus::alloc::boxed::Box<self::InitialSpawnStart>),

                /// The variant of type `SubmitInitialSpawns` in the union `Message`
                SubmitInitialSpawns(::planus::alloc::boxed::Box<self::SubmitInitialSpawns>),

                /// The variant of type `InitialSpawnResult` in the union `Message`
                InitialSpawnResult(::planus::alloc::boxed::Box<self::InitialSpawnResult>),

                /// The variant of type `RoundStartSignal` in the union `Message`
                RoundStartSignal(::planus::alloc::boxed::Box<self::RoundStartSignal>),

                /// The variant of type `SubmitCommandStream` in the union `Message`
                SubmitCommandStream(::planus::alloc::boxed::Box<self::SubmitCommandStream>),

                /// The variant of type `SubmissionResult` in the union `Message`
                SubmissionResult(::planus::alloc::boxed::Box<self::SubmissionResult>),

                /// The variant of type `CommitRound` in the union `Message`
                CommitRound(::planus::alloc::boxed::Box<self::CommitRound>),

                /// The variant of type `RoundClosed` in the union `Message`
                RoundClosed(::planus::alloc::boxed::Box<self::RoundClosed>),

                /// The variant of type `GameEnd` in the union `Message`
                GameEnd(::planus::alloc::boxed::Box<self::GameEnd>),
            }

            impl Message {
                /// Creates a [MessageBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> MessageBuilder<::planus::Uninitialized> {
                    MessageBuilder(::planus::Uninitialized)
                }

                #[inline]
                pub fn create_client_hello(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::ClientHello>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(1, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_server_hello(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::ServerHello>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(2, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_initial_spawn_start(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::InitialSpawnStart>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(3, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_submit_initial_spawns(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::SubmitInitialSpawns>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(4, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_initial_spawn_result(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::InitialSpawnResult>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(5, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_round_start_signal(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::RoundStartSignal>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(6, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_submit_command_stream(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::SubmitCommandStream>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(7, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_submission_result(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::SubmissionResult>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(8, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_commit_round(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::CommitRound>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(9, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_round_closed(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::RoundClosed>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(10, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_game_end(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::GameEnd>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(11, value.prepare(builder).downcast())
                }
            }

            impl ::planus::WriteAsUnion<Message> for Message {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::UnionOffset<Self> {
                    match self {
                        Self::ClientHello(value) => Self::create_client_hello(builder, value),
                        Self::ServerHello(value) => Self::create_server_hello(builder, value),
                        Self::InitialSpawnStart(value) => {
                            Self::create_initial_spawn_start(builder, value)
                        }
                        Self::SubmitInitialSpawns(value) => {
                            Self::create_submit_initial_spawns(builder, value)
                        }
                        Self::InitialSpawnResult(value) => {
                            Self::create_initial_spawn_result(builder, value)
                        }
                        Self::RoundStartSignal(value) => {
                            Self::create_round_start_signal(builder, value)
                        }
                        Self::SubmitCommandStream(value) => {
                            Self::create_submit_command_stream(builder, value)
                        }
                        Self::SubmissionResult(value) => {
                            Self::create_submission_result(builder, value)
                        }
                        Self::CommitRound(value) => Self::create_commit_round(builder, value),
                        Self::RoundClosed(value) => Self::create_round_closed(builder, value),
                        Self::GameEnd(value) => Self::create_game_end(builder, value),
                    }
                }
            }

            impl ::planus::WriteAsOptionalUnion<Message> for Message {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Self>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }

            /// Builder for serializing an instance of the [Message] type.
            ///
            /// Can be created using the [Message::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct MessageBuilder<T>(T);

            impl MessageBuilder<::planus::Uninitialized> {
                /// Creates an instance of the [`ClientHello` variant](Message#variant.ClientHello).
                #[inline]
                pub fn client_hello<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<1, T>>
                where
                    T: ::planus::WriteAsOffset<self::ClientHello>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`ServerHello` variant](Message#variant.ServerHello).
                #[inline]
                pub fn server_hello<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<2, T>>
                where
                    T: ::planus::WriteAsOffset<self::ServerHello>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`InitialSpawnStart` variant](Message#variant.InitialSpawnStart).
                #[inline]
                pub fn initial_spawn_start<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<3, T>>
                where
                    T: ::planus::WriteAsOffset<self::InitialSpawnStart>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`SubmitInitialSpawns` variant](Message#variant.SubmitInitialSpawns).
                #[inline]
                pub fn submit_initial_spawns<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<4, T>>
                where
                    T: ::planus::WriteAsOffset<self::SubmitInitialSpawns>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`InitialSpawnResult` variant](Message#variant.InitialSpawnResult).
                #[inline]
                pub fn initial_spawn_result<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<5, T>>
                where
                    T: ::planus::WriteAsOffset<self::InitialSpawnResult>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`RoundStartSignal` variant](Message#variant.RoundStartSignal).
                #[inline]
                pub fn round_start_signal<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<6, T>>
                where
                    T: ::planus::WriteAsOffset<self::RoundStartSignal>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`SubmitCommandStream` variant](Message#variant.SubmitCommandStream).
                #[inline]
                pub fn submit_command_stream<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<7, T>>
                where
                    T: ::planus::WriteAsOffset<self::SubmitCommandStream>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`SubmissionResult` variant](Message#variant.SubmissionResult).
                #[inline]
                pub fn submission_result<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<8, T>>
                where
                    T: ::planus::WriteAsOffset<self::SubmissionResult>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`CommitRound` variant](Message#variant.CommitRound).
                #[inline]
                pub fn commit_round<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<9, T>>
                where
                    T: ::planus::WriteAsOffset<self::CommitRound>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`RoundClosed` variant](Message#variant.RoundClosed).
                #[inline]
                pub fn round_closed<T>(
                    self,
                    value: T,
                ) -> MessageBuilder<::planus::Initialized<10, T>>
                where
                    T: ::planus::WriteAsOffset<self::RoundClosed>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`GameEnd` variant](Message#variant.GameEnd).
                #[inline]
                pub fn game_end<T>(self, value: T) -> MessageBuilder<::planus::Initialized<11, T>>
                where
                    T: ::planus::WriteAsOffset<self::GameEnd>,
                {
                    MessageBuilder(::planus::Initialized(value))
                }
            }

            impl<const N: u8, T> MessageBuilder<::planus::Initialized<N, T>> {
                /// Finish writing the builder to get an [UnionOffset](::planus::UnionOffset) to a serialized [Message].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message>
                where
                    Self: ::planus::WriteAsUnion<Message>,
                {
                    ::planus::WriteAsUnion::prepare(&self, builder)
                }
            }

            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<1, T>>
            where
                T: ::planus::WriteAsOffset<self::ClientHello>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(1, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<1, T>>
            where
                T: ::planus::WriteAsOffset<self::ClientHello>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<2, T>>
            where
                T: ::planus::WriteAsOffset<self::ServerHello>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(2, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<2, T>>
            where
                T: ::planus::WriteAsOffset<self::ServerHello>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<3, T>>
            where
                T: ::planus::WriteAsOffset<self::InitialSpawnStart>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(3, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<3, T>>
            where
                T: ::planus::WriteAsOffset<self::InitialSpawnStart>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<4, T>>
            where
                T: ::planus::WriteAsOffset<self::SubmitInitialSpawns>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(4, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<4, T>>
            where
                T: ::planus::WriteAsOffset<self::SubmitInitialSpawns>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<5, T>>
            where
                T: ::planus::WriteAsOffset<self::InitialSpawnResult>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(5, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<5, T>>
            where
                T: ::planus::WriteAsOffset<self::InitialSpawnResult>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<6, T>>
            where
                T: ::planus::WriteAsOffset<self::RoundStartSignal>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(6, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<6, T>>
            where
                T: ::planus::WriteAsOffset<self::RoundStartSignal>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<7, T>>
            where
                T: ::planus::WriteAsOffset<self::SubmitCommandStream>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(7, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<7, T>>
            where
                T: ::planus::WriteAsOffset<self::SubmitCommandStream>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<8, T>>
            where
                T: ::planus::WriteAsOffset<self::SubmissionResult>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(8, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<8, T>>
            where
                T: ::planus::WriteAsOffset<self::SubmissionResult>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<9, T>>
            where
                T: ::planus::WriteAsOffset<self::CommitRound>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(9, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<9, T>>
            where
                T: ::planus::WriteAsOffset<self::CommitRound>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<10, T>>
            where
                T: ::planus::WriteAsOffset<self::RoundClosed>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(10, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<10, T>>
            where
                T: ::planus::WriteAsOffset<self::RoundClosed>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<Message> for MessageBuilder<::planus::Initialized<11, T>>
            where
                T: ::planus::WriteAsOffset<self::GameEnd>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<Message> {
                    ::planus::UnionOffset::new(11, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<Message> for MessageBuilder<::planus::Initialized<11, T>>
            where
                T: ::planus::WriteAsOffset<self::GameEnd>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Message>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }

            /// Reference to a deserialized [Message].
            #[derive(Copy, Clone, Debug)]
            pub enum MessageRef<'a> {
                ClientHello(self::ClientHelloRef<'a>),
                ServerHello(self::ServerHelloRef<'a>),
                InitialSpawnStart(self::InitialSpawnStartRef<'a>),
                SubmitInitialSpawns(self::SubmitInitialSpawnsRef<'a>),
                InitialSpawnResult(self::InitialSpawnResultRef<'a>),
                RoundStartSignal(self::RoundStartSignalRef<'a>),
                SubmitCommandStream(self::SubmitCommandStreamRef<'a>),
                SubmissionResult(self::SubmissionResultRef<'a>),
                CommitRound(self::CommitRoundRef<'a>),
                RoundClosed(self::RoundClosedRef<'a>),
                GameEnd(self::GameEndRef<'a>),
            }

            impl<'a> ::core::convert::TryFrom<MessageRef<'a>> for Message {
                type Error = ::planus::Error;

                fn try_from(value: MessageRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(match value {
                        MessageRef::ClientHello(value) => {
                            Self::ClientHello(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::ServerHello(value) => {
                            Self::ServerHello(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::InitialSpawnStart(value) => {
                            Self::InitialSpawnStart(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::SubmitInitialSpawns(value) => {
                            Self::SubmitInitialSpawns(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::InitialSpawnResult(value) => {
                            Self::InitialSpawnResult(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::RoundStartSignal(value) => {
                            Self::RoundStartSignal(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::SubmitCommandStream(value) => {
                            Self::SubmitCommandStream(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::SubmissionResult(value) => {
                            Self::SubmissionResult(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::CommitRound(value) => {
                            Self::CommitRound(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::RoundClosed(value) => {
                            Self::RoundClosed(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        MessageRef::GameEnd(value) => {
                            Self::GameEnd(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }
                    })
                }
            }

            impl<'a> ::planus::TableReadUnion<'a> for MessageRef<'a> {
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    tag: u8,
                    field_offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    match tag {
                        1 => ::core::result::Result::Ok(Self::ClientHello(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        2 => ::core::result::Result::Ok(Self::ServerHello(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        3 => ::core::result::Result::Ok(Self::InitialSpawnStart(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        4 => ::core::result::Result::Ok(Self::SubmitInitialSpawns(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        5 => ::core::result::Result::Ok(Self::InitialSpawnResult(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        6 => ::core::result::Result::Ok(Self::RoundStartSignal(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        7 => ::core::result::Result::Ok(Self::SubmitCommandStream(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        8 => ::core::result::Result::Ok(Self::SubmissionResult(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        9 => ::core::result::Result::Ok(Self::CommitRound(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        10 => ::core::result::Result::Ok(Self::RoundClosed(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        11 => ::core::result::Result::Ok(Self::GameEnd(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        _ => ::core::result::Result::Err(
                            ::planus::errors::ErrorKind::UnknownUnionTag { tag },
                        ),
                    }
                }
            }

            impl<'a> ::planus::VectorReadUnion<'a> for MessageRef<'a> {
                const VECTOR_NAME: &'static str = "[MessageRef]";
            }

            ///  Frame root. `protocol_version` must equal the server's
            ///  PROTOCOL_VERSION at handshake; later messages carry no version fields
            ///  (a running client declaring wrong versions is an envelope reject).
            ///
            /// Generated from these locations:
            /// * Table `ProtocolEnvelope` in the file `crates/arena-protocol/schemas/protocol.fbs:36`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct ProtocolEnvelope {
                /// The field `protocol_version` in the table `ProtocolEnvelope`
                pub protocol_version: u32,
                /// The field `message` in the table `ProtocolEnvelope`
                pub message: ::core::option::Option<self::Message>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for ProtocolEnvelope {
                fn default() -> Self {
                    Self {
                        protocol_version: 1,
                        message: ::core::default::Default::default(),
                    }
                }
            }

            impl ProtocolEnvelope {
                /// Creates a [ProtocolEnvelopeBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> ProtocolEnvelopeBuilder<()> {
                    ProtocolEnvelopeBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_protocol_version: impl ::planus::WriteAsDefault<u32, u32>,
                    field_message: impl ::planus::WriteAsOptionalUnion<self::Message>,
                ) -> ::planus::Offset<Self> {
                    let prepared_protocol_version = field_protocol_version.prepare(builder, &1);
                    let prepared_message = field_message.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<10> =
                        ::core::default::Default::default();
                    if prepared_protocol_version.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }
                    if prepared_message.is_some() {
                        table_writer.write_entry::<::planus::Offset<self::Message>>(2);
                    }
                    if prepared_message.is_some() {
                        table_writer.write_entry::<u8>(1);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_protocol_version) =
                                prepared_protocol_version
                            {
                                object_writer.write::<_, _, 4>(&prepared_protocol_version);
                            }
                            if let ::core::option::Option::Some(prepared_message) = prepared_message
                            {
                                object_writer.write::<_, _, 4>(&prepared_message.offset());
                            }
                            if let ::core::option::Option::Some(prepared_message) = prepared_message
                            {
                                object_writer.write::<_, _, 1>(&prepared_message.tag());
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<ProtocolEnvelope>> for ProtocolEnvelope {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ProtocolEnvelope> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<ProtocolEnvelope>> for ProtocolEnvelope {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ProtocolEnvelope>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<ProtocolEnvelope> for ProtocolEnvelope {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ProtocolEnvelope> {
                    ProtocolEnvelope::create(builder, self.protocol_version, &self.message)
                }
            }

            /// Builder for serializing an instance of the [ProtocolEnvelope] type.
            ///
            /// Can be created using the [ProtocolEnvelope::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct ProtocolEnvelopeBuilder<State>(State);

            impl ProtocolEnvelopeBuilder<()> {
                /// Setter for the [`protocol_version` field](ProtocolEnvelope#structfield.protocol_version).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn protocol_version<T0>(self, value: T0) -> ProtocolEnvelopeBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    ProtocolEnvelopeBuilder((value,))
                }

                /// Sets the [`protocol_version` field](ProtocolEnvelope#structfield.protocol_version) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn protocol_version_as_default(
                    self,
                ) -> ProtocolEnvelopeBuilder<(::planus::DefaultValue,)> {
                    self.protocol_version(::planus::DefaultValue)
                }
            }

            impl<T0> ProtocolEnvelopeBuilder<(T0,)> {
                /// Setter for the [`message` field](ProtocolEnvelope#structfield.message).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn message<T1>(self, value: T1) -> ProtocolEnvelopeBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsOptionalUnion<self::Message>,
                {
                    let (v0,) = self.0;
                    ProtocolEnvelopeBuilder((v0, value))
                }

                /// Sets the [`message` field](ProtocolEnvelope#structfield.message) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn message_as_null(self) -> ProtocolEnvelopeBuilder<(T0, ())> {
                    self.message(())
                }
            }

            impl<T0, T1> ProtocolEnvelopeBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ProtocolEnvelope].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ProtocolEnvelope>
                where
                    Self: ::planus::WriteAsOffset<ProtocolEnvelope>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsOptionalUnion<self::Message>,
            > ::planus::WriteAs<::planus::Offset<ProtocolEnvelope>>
                for ProtocolEnvelopeBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ProtocolEnvelope>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ProtocolEnvelope> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsOptionalUnion<self::Message>,
            > ::planus::WriteAsOptional<::planus::Offset<ProtocolEnvelope>>
                for ProtocolEnvelopeBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ProtocolEnvelope>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ProtocolEnvelope>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsOptionalUnion<self::Message>,
            > ::planus::WriteAsOffset<ProtocolEnvelope> for ProtocolEnvelopeBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ProtocolEnvelope> {
                    let (v0, v1) = &self.0;
                    ProtocolEnvelope::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [ProtocolEnvelope].
            #[derive(Copy, Clone)]
            pub struct ProtocolEnvelopeRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> ProtocolEnvelopeRef<'a> {
                /// Getter for the [`protocol_version` field](ProtocolEnvelope#structfield.protocol_version).
                #[inline]
                pub fn protocol_version(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(0, "ProtocolEnvelope", "protocol_version")?
                            .unwrap_or(1),
                    )
                }

                /// Getter for the [`message` field](ProtocolEnvelope#structfield.message).
                #[inline]
                pub fn message(
                    &self,
                ) -> ::planus::Result<::core::option::Option<self::MessageRef<'a>>>
                {
                    self.0.access_union(1, "ProtocolEnvelope", "message")
                }
            }

            impl<'a> ::core::fmt::Debug for ProtocolEnvelopeRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("ProtocolEnvelopeRef");
                    f.field("protocol_version", &self.protocol_version());
                    if let ::core::option::Option::Some(field_message) = self.message().transpose()
                    {
                        f.field("message", &field_message);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<ProtocolEnvelopeRef<'a>> for ProtocolEnvelope {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: ProtocolEnvelopeRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        protocol_version: ::core::convert::TryInto::try_into(
                            value.protocol_version()?,
                        )?,
                        message: if let ::core::option::Option::Some(message) = value.message()? {
                            ::core::option::Option::Some(::core::convert::TryInto::try_into(
                                message,
                            )?)
                        } else {
                            ::core::option::Option::None
                        },
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for ProtocolEnvelopeRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for ProtocolEnvelopeRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[ProtocolEnvelopeRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<ProtocolEnvelope>> for ProtocolEnvelope {
                type Value = ::planus::Offset<ProtocolEnvelope>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<ProtocolEnvelope>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for ProtocolEnvelopeRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[ProtocolEnvelopeRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Client -> Server, first frame on the connection. All three version
            ///  fields must EXACTLY match the server constants (no negotiation).
            ///
            /// Generated from these locations:
            /// * Table `ClientHello` in the file `crates/arena-protocol/schemas/protocol.fbs:43`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct ClientHello {
                /// The field `client_name` in the table `ClientHello`
                pub client_name: ::core::option::Option<::planus::alloc::string::String>,
                /// The field `client_version` in the table `ClientHello`
                pub client_version: ::core::option::Option<::planus::alloc::string::String>,
                /// The field `protocol_version` in the table `ClientHello`
                pub protocol_version: u32,
                /// The field `public_snapshot_version` in the table `ClientHello`
                pub public_snapshot_version: u32,
                /// The field `private_snapshot_version` in the table `ClientHello`
                pub private_snapshot_version: u32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for ClientHello {
                fn default() -> Self {
                    Self {
                        client_name: ::core::default::Default::default(),
                        client_version: ::core::default::Default::default(),
                        protocol_version: 0,
                        public_snapshot_version: 0,
                        private_snapshot_version: 0,
                    }
                }
            }

            impl ClientHello {
                /// Creates a [ClientHelloBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> ClientHelloBuilder<()> {
                    ClientHelloBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_client_name: impl ::planus::WriteAsOptional<
                        ::planus::Offset<::core::primitive::str>,
                    >,
                    field_client_version: impl ::planus::WriteAsOptional<
                        ::planus::Offset<::core::primitive::str>,
                    >,
                    field_protocol_version: impl ::planus::WriteAsDefault<u32, u32>,
                    field_public_snapshot_version: impl ::planus::WriteAsDefault<u32, u32>,
                    field_private_snapshot_version: impl ::planus::WriteAsDefault<u32, u32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_client_name = field_client_name.prepare(builder);
                    let prepared_client_version = field_client_version.prepare(builder);
                    let prepared_protocol_version = field_protocol_version.prepare(builder, &0);
                    let prepared_public_snapshot_version =
                        field_public_snapshot_version.prepare(builder, &0);
                    let prepared_private_snapshot_version =
                        field_private_snapshot_version.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<14> =
                        ::core::default::Default::default();
                    if prepared_client_name.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(0);
                    }
                    if prepared_client_version.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(1);
                    }
                    if prepared_protocol_version.is_some() {
                        table_writer.write_entry::<u32>(2);
                    }
                    if prepared_public_snapshot_version.is_some() {
                        table_writer.write_entry::<u32>(3);
                    }
                    if prepared_private_snapshot_version.is_some() {
                        table_writer.write_entry::<u32>(4);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_client_name) =
                                prepared_client_name
                            {
                                object_writer.write::<_, _, 4>(&prepared_client_name);
                            }
                            if let ::core::option::Option::Some(prepared_client_version) =
                                prepared_client_version
                            {
                                object_writer.write::<_, _, 4>(&prepared_client_version);
                            }
                            if let ::core::option::Option::Some(prepared_protocol_version) =
                                prepared_protocol_version
                            {
                                object_writer.write::<_, _, 4>(&prepared_protocol_version);
                            }
                            if let ::core::option::Option::Some(prepared_public_snapshot_version) =
                                prepared_public_snapshot_version
                            {
                                object_writer.write::<_, _, 4>(&prepared_public_snapshot_version);
                            }
                            if let ::core::option::Option::Some(prepared_private_snapshot_version) =
                                prepared_private_snapshot_version
                            {
                                object_writer.write::<_, _, 4>(&prepared_private_snapshot_version);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<ClientHello>> for ClientHello {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ClientHello> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<ClientHello>> for ClientHello {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ClientHello>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<ClientHello> for ClientHello {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ClientHello> {
                    ClientHello::create(
                        builder,
                        &self.client_name,
                        &self.client_version,
                        self.protocol_version,
                        self.public_snapshot_version,
                        self.private_snapshot_version,
                    )
                }
            }

            /// Builder for serializing an instance of the [ClientHello] type.
            ///
            /// Can be created using the [ClientHello::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct ClientHelloBuilder<State>(State);

            impl ClientHelloBuilder<()> {
                /// Setter for the [`client_name` field](ClientHello#structfield.client_name).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn client_name<T0>(self, value: T0) -> ClientHelloBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    ClientHelloBuilder((value,))
                }

                /// Sets the [`client_name` field](ClientHello#structfield.client_name) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn client_name_as_null(self) -> ClientHelloBuilder<((),)> {
                    self.client_name(())
                }
            }

            impl<T0> ClientHelloBuilder<(T0,)> {
                /// Setter for the [`client_version` field](ClientHello#structfield.client_version).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn client_version<T1>(self, value: T1) -> ClientHelloBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    let (v0,) = self.0;
                    ClientHelloBuilder((v0, value))
                }

                /// Sets the [`client_version` field](ClientHello#structfield.client_version) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn client_version_as_null(self) -> ClientHelloBuilder<(T0, ())> {
                    self.client_version(())
                }
            }

            impl<T0, T1> ClientHelloBuilder<(T0, T1)> {
                /// Setter for the [`protocol_version` field](ClientHello#structfield.protocol_version).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn protocol_version<T2>(self, value: T2) -> ClientHelloBuilder<(T0, T1, T2)>
                where
                    T2: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1) = self.0;
                    ClientHelloBuilder((v0, v1, value))
                }

                /// Sets the [`protocol_version` field](ClientHello#structfield.protocol_version) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn protocol_version_as_default(
                    self,
                ) -> ClientHelloBuilder<(T0, T1, ::planus::DefaultValue)> {
                    self.protocol_version(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2> ClientHelloBuilder<(T0, T1, T2)> {
                /// Setter for the [`public_snapshot_version` field](ClientHello#structfield.public_snapshot_version).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn public_snapshot_version<T3>(
                    self,
                    value: T3,
                ) -> ClientHelloBuilder<(T0, T1, T2, T3)>
                where
                    T3: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2) = self.0;
                    ClientHelloBuilder((v0, v1, v2, value))
                }

                /// Sets the [`public_snapshot_version` field](ClientHello#structfield.public_snapshot_version) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn public_snapshot_version_as_default(
                    self,
                ) -> ClientHelloBuilder<(T0, T1, T2, ::planus::DefaultValue)> {
                    self.public_snapshot_version(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3> ClientHelloBuilder<(T0, T1, T2, T3)> {
                /// Setter for the [`private_snapshot_version` field](ClientHello#structfield.private_snapshot_version).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn private_snapshot_version<T4>(
                    self,
                    value: T4,
                ) -> ClientHelloBuilder<(T0, T1, T2, T3, T4)>
                where
                    T4: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3) = self.0;
                    ClientHelloBuilder((v0, v1, v2, v3, value))
                }

                /// Sets the [`private_snapshot_version` field](ClientHello#structfield.private_snapshot_version) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn private_snapshot_version_as_default(
                    self,
                ) -> ClientHelloBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)> {
                    self.private_snapshot_version(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4> ClientHelloBuilder<(T0, T1, T2, T3, T4)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ClientHello].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ClientHello>
                where
                    Self: ::planus::WriteAsOffset<ClientHello>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                T2: ::planus::WriteAsDefault<u32, u32>,
                T3: ::planus::WriteAsDefault<u32, u32>,
                T4: ::planus::WriteAsDefault<u32, u32>,
            > ::planus::WriteAs<::planus::Offset<ClientHello>>
                for ClientHelloBuilder<(T0, T1, T2, T3, T4)>
            {
                type Prepared = ::planus::Offset<ClientHello>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ClientHello> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                T2: ::planus::WriteAsDefault<u32, u32>,
                T3: ::planus::WriteAsDefault<u32, u32>,
                T4: ::planus::WriteAsDefault<u32, u32>,
            > ::planus::WriteAsOptional<::planus::Offset<ClientHello>>
                for ClientHelloBuilder<(T0, T1, T2, T3, T4)>
            {
                type Prepared = ::planus::Offset<ClientHello>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ClientHello>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                T2: ::planus::WriteAsDefault<u32, u32>,
                T3: ::planus::WriteAsDefault<u32, u32>,
                T4: ::planus::WriteAsDefault<u32, u32>,
            > ::planus::WriteAsOffset<ClientHello> for ClientHelloBuilder<(T0, T1, T2, T3, T4)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ClientHello> {
                    let (v0, v1, v2, v3, v4) = &self.0;
                    ClientHello::create(builder, v0, v1, v2, v3, v4)
                }
            }

            /// Reference to a deserialized [ClientHello].
            #[derive(Copy, Clone)]
            pub struct ClientHelloRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> ClientHelloRef<'a> {
                /// Getter for the [`client_name` field](ClientHello#structfield.client_name).
                #[inline]
                pub fn client_name(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(0, "ClientHello", "client_name")
                }

                /// Getter for the [`client_version` field](ClientHello#structfield.client_version).
                #[inline]
                pub fn client_version(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(1, "ClientHello", "client_version")
                }

                /// Getter for the [`protocol_version` field](ClientHello#structfield.protocol_version).
                #[inline]
                pub fn protocol_version(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(2, "ClientHello", "protocol_version")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`public_snapshot_version` field](ClientHello#structfield.public_snapshot_version).
                #[inline]
                pub fn public_snapshot_version(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(3, "ClientHello", "public_snapshot_version")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`private_snapshot_version` field](ClientHello#structfield.private_snapshot_version).
                #[inline]
                pub fn private_snapshot_version(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(4, "ClientHello", "private_snapshot_version")?
                            .unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for ClientHelloRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("ClientHelloRef");
                    if let ::core::option::Option::Some(field_client_name) =
                        self.client_name().transpose()
                    {
                        f.field("client_name", &field_client_name);
                    }
                    if let ::core::option::Option::Some(field_client_version) =
                        self.client_version().transpose()
                    {
                        f.field("client_version", &field_client_version);
                    }
                    f.field("protocol_version", &self.protocol_version());
                    f.field("public_snapshot_version", &self.public_snapshot_version());
                    f.field("private_snapshot_version", &self.private_snapshot_version());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<ClientHelloRef<'a>> for ClientHello {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: ClientHelloRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        client_name: value.client_name()?.map(::core::convert::Into::into),
                        client_version: value.client_version()?.map(::core::convert::Into::into),
                        protocol_version: ::core::convert::TryInto::try_into(
                            value.protocol_version()?,
                        )?,
                        public_snapshot_version: ::core::convert::TryInto::try_into(
                            value.public_snapshot_version()?,
                        )?,
                        private_snapshot_version: ::core::convert::TryInto::try_into(
                            value.private_snapshot_version()?,
                        )?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for ClientHelloRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for ClientHelloRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[ClientHelloRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<ClientHello>> for ClientHello {
                type Value = ::planus::Offset<ClientHello>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<ClientHello>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for ClientHelloRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[ClientHelloRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Server -> Client. status: 0 = ok (faction assigned, match starting),
            ///  1 = version mismatch (startup aborts, unscored), 2 = duplicate /
            ///  surplus connection (closed immediately).
            ///
            /// Generated from these locations:
            /// * Table `ServerHello` in the file `crates/arena-protocol/schemas/protocol.fbs:54`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct ServerHello {
                /// The field `status` in the table `ServerHello`
                pub status: u8,
                /// The field `assigned_faction` in the table `ServerHello`
                pub assigned_faction: self::Faction,
                /// The field `match_id` in the table `ServerHello`
                pub match_id: ::core::option::Option<::planus::alloc::string::String>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for ServerHello {
                fn default() -> Self {
                    Self {
                        status: 0,
                        assigned_faction: self::Faction::Enl,
                        match_id: ::core::default::Default::default(),
                    }
                }
            }

            impl ServerHello {
                /// Creates a [ServerHelloBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> ServerHelloBuilder<()> {
                    ServerHelloBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_status: impl ::planus::WriteAsDefault<u8, u8>,
                    field_assigned_faction: impl ::planus::WriteAsDefault<self::Faction, self::Faction>,
                    field_match_id: impl ::planus::WriteAsOptional<
                        ::planus::Offset<::core::primitive::str>,
                    >,
                ) -> ::planus::Offset<Self> {
                    let prepared_status = field_status.prepare(builder, &0);
                    let prepared_assigned_faction =
                        field_assigned_faction.prepare(builder, &self::Faction::Enl);
                    let prepared_match_id = field_match_id.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<10> =
                        ::core::default::Default::default();
                    if prepared_match_id.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(2);
                    }
                    if prepared_status.is_some() {
                        table_writer.write_entry::<u8>(0);
                    }
                    if prepared_assigned_faction.is_some() {
                        table_writer.write_entry::<self::Faction>(1);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_match_id) =
                                prepared_match_id
                            {
                                object_writer.write::<_, _, 4>(&prepared_match_id);
                            }
                            if let ::core::option::Option::Some(prepared_status) = prepared_status {
                                object_writer.write::<_, _, 1>(&prepared_status);
                            }
                            if let ::core::option::Option::Some(prepared_assigned_faction) =
                                prepared_assigned_faction
                            {
                                object_writer.write::<_, _, 1>(&prepared_assigned_faction);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<ServerHello>> for ServerHello {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ServerHello> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<ServerHello>> for ServerHello {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ServerHello>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<ServerHello> for ServerHello {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ServerHello> {
                    ServerHello::create(builder, self.status, self.assigned_faction, &self.match_id)
                }
            }

            /// Builder for serializing an instance of the [ServerHello] type.
            ///
            /// Can be created using the [ServerHello::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct ServerHelloBuilder<State>(State);

            impl ServerHelloBuilder<()> {
                /// Setter for the [`status` field](ServerHello#structfield.status).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn status<T0>(self, value: T0) -> ServerHelloBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u8, u8>,
                {
                    ServerHelloBuilder((value,))
                }

                /// Sets the [`status` field](ServerHello#structfield.status) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn status_as_default(self) -> ServerHelloBuilder<(::planus::DefaultValue,)> {
                    self.status(::planus::DefaultValue)
                }
            }

            impl<T0> ServerHelloBuilder<(T0,)> {
                /// Setter for the [`assigned_faction` field](ServerHello#structfield.assigned_faction).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn assigned_faction<T1>(self, value: T1) -> ServerHelloBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<self::Faction, self::Faction>,
                {
                    let (v0,) = self.0;
                    ServerHelloBuilder((v0, value))
                }

                /// Sets the [`assigned_faction` field](ServerHello#structfield.assigned_faction) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn assigned_faction_as_default(
                    self,
                ) -> ServerHelloBuilder<(T0, ::planus::DefaultValue)> {
                    self.assigned_faction(::planus::DefaultValue)
                }
            }

            impl<T0, T1> ServerHelloBuilder<(T0, T1)> {
                /// Setter for the [`match_id` field](ServerHello#structfield.match_id).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn match_id<T2>(self, value: T2) -> ServerHelloBuilder<(T0, T1, T2)>
                where
                    T2: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    let (v0, v1) = self.0;
                    ServerHelloBuilder((v0, v1, value))
                }

                /// Sets the [`match_id` field](ServerHello#structfield.match_id) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn match_id_as_null(self) -> ServerHelloBuilder<(T0, T1, ())> {
                    self.match_id(())
                }
            }

            impl<T0, T1, T2> ServerHelloBuilder<(T0, T1, T2)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ServerHello].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ServerHello>
                where
                    Self: ::planus::WriteAsOffset<ServerHello>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsDefault<self::Faction, self::Faction>,
                T2: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
            > ::planus::WriteAs<::planus::Offset<ServerHello>>
                for ServerHelloBuilder<(T0, T1, T2)>
            {
                type Prepared = ::planus::Offset<ServerHello>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ServerHello> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsDefault<self::Faction, self::Faction>,
                T2: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
            > ::planus::WriteAsOptional<::planus::Offset<ServerHello>>
                for ServerHelloBuilder<(T0, T1, T2)>
            {
                type Prepared = ::planus::Offset<ServerHello>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ServerHello>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsDefault<self::Faction, self::Faction>,
                T2: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
            > ::planus::WriteAsOffset<ServerHello> for ServerHelloBuilder<(T0, T1, T2)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ServerHello> {
                    let (v0, v1, v2) = &self.0;
                    ServerHello::create(builder, v0, v1, v2)
                }
            }

            /// Reference to a deserialized [ServerHello].
            #[derive(Copy, Clone)]
            pub struct ServerHelloRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> ServerHelloRef<'a> {
                /// Getter for the [`status` field](ServerHello#structfield.status).
                #[inline]
                pub fn status(&self) -> ::planus::Result<u8> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "ServerHello", "status")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`assigned_faction` field](ServerHello#structfield.assigned_faction).
                #[inline]
                pub fn assigned_faction(&self) -> ::planus::Result<self::Faction> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(1, "ServerHello", "assigned_faction")?
                            .unwrap_or(self::Faction::Enl),
                    )
                }

                /// Getter for the [`match_id` field](ServerHello#structfield.match_id).
                #[inline]
                pub fn match_id(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(2, "ServerHello", "match_id")
                }
            }

            impl<'a> ::core::fmt::Debug for ServerHelloRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("ServerHelloRef");
                    f.field("status", &self.status());
                    f.field("assigned_faction", &self.assigned_faction());
                    if let ::core::option::Option::Some(field_match_id) =
                        self.match_id().transpose()
                    {
                        f.field("match_id", &field_match_id);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<ServerHelloRef<'a>> for ServerHello {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: ServerHelloRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        status: ::core::convert::TryInto::try_into(value.status()?)?,
                        assigned_faction: ::core::convert::TryInto::try_into(
                            value.assigned_faction()?,
                        )?,
                        match_id: value.match_id()?.map(::core::convert::Into::into),
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for ServerHelloRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for ServerHelloRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[ServerHelloRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<ServerHello>> for ServerHello {
                type Value = ::planus::Offset<ServerHello>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<ServerHello>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for ServerHelloRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[ServerHelloRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Full rule parameter echo (every GameConfig field, declaration order).
            ///  Clients read constants from here, never from local defaults.
            ///
            /// Generated from these locations:
            /// * Table `MatchInfo` in the file `crates/arena-protocol/schemas/protocol.fbs:62`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct MatchInfo {
                /// The field `total_rounds` in the table `MatchInfo`
                pub total_rounds: u32,
                /// The field `initial_spawn_window_ms` in the table `MatchInfo`
                pub initial_spawn_window_ms: u32,
                /// The field `decision_window_ms` in the table `MatchInfo`
                pub decision_window_ms: u32,
                /// The field `priority_bucket_ms` in the table `MatchInfo`
                pub priority_bucket_ms: u32,
                /// The field `epoch_length` in the table `MatchInfo`
                pub epoch_length: u32,
                /// The field `map_width_cm` in the table `MatchInfo`
                pub map_width_cm: i32,
                /// The field `map_height_cm` in the table `MatchInfo`
                pub map_height_cm: i32,
                /// The field `portal_count` in the table `MatchInfo`
                pub portal_count: u32,
                /// The field `portal_min_distance_cm` in the table `MatchInfo`
                pub portal_min_distance_cm: u32,
                /// The field `clustered_portal_ratio_permille` in the table `MatchInfo`
                pub clustered_portal_ratio_permille: u32,
                /// The field `portal_cluster_count` in the table `MatchInfo`
                pub portal_cluster_count: u32,
                /// The field `initial_agents_per_faction` in the table `MatchInfo`
                pub initial_agents_per_faction: u32,
                /// The field `reinforcement_interval_rounds` in the table `MatchInfo`
                pub reinforcement_interval_rounds: u32,
                /// The field `reinforcement_agents_per_faction` in the table `MatchInfo`
                pub reinforcement_agents_per_faction: u32,
                /// The field `move_budget_cm` in the table `MatchInfo`
                pub move_budget_cm: u32,
                /// The field `portal_operation_range_cm` in the table `MatchInfo`
                pub portal_operation_range_cm: u32,
                /// The field `transfer_range_cm` in the table `MatchInfo`
                pub transfer_range_cm: u32,
                /// The field `initial_supply` in the table `MatchInfo`
                pub initial_supply: u32,
                /// The field `max_supply` in the table `MatchInfo`
                pub max_supply: u32,
                /// The field `portal_max_hp` in the table `MatchInfo`
                pub portal_max_hp: u32,
                /// The field `attack_damage` in the table `MatchInfo`
                pub attack_damage: u32,
                /// The field `recharge_amount` in the table `MatchInfo`
                pub recharge_amount: u32,
                /// The field `recharge_cost_supply` in the table `MatchInfo`
                pub recharge_cost_supply: u32,
                /// The field `hack_supply_yield` in the table `MatchInfo`
                pub hack_supply_yield: u32,
                /// The field `hack_key_yield_min` in the table `MatchInfo`
                pub hack_key_yield_min: u32,
                /// The field `hack_key_yield_max` in the table `MatchInfo`
                pub hack_key_yield_max: u32,
                /// The field `hack_cooldown_rounds` in the table `MatchInfo`
                pub hack_cooldown_rounds: u32,
                /// The field `hack_limit_per_epoch` in the table `MatchInfo`
                pub hack_limit_per_epoch: u32,
                /// The field `max_link_distance_cm` in the table `MatchInfo`
                pub max_link_distance_cm: u32,
                /// The field `max_commands_per_agent_round` in the table `MatchInfo`
                pub max_commands_per_agent_round: u32,
                ///  Largest legal frame payload the transport accepts (65,535). The
                ///  server's GameConfig stores the EXCLUSIVE ceiling (65,536, ADR-0008);
                ///  this field publishes the enforced LEGAL maximum — one below the
                ///  ceiling — so a client sizing its largest frame to exactly the
                ///  published value is legal (D62).
                pub max_frame_payload_bytes: u32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for MatchInfo {
                fn default() -> Self {
                    Self {
                        total_rounds: 0,
                        initial_spawn_window_ms: 0,
                        decision_window_ms: 0,
                        priority_bucket_ms: 0,
                        epoch_length: 0,
                        map_width_cm: 0,
                        map_height_cm: 0,
                        portal_count: 0,
                        portal_min_distance_cm: 0,
                        clustered_portal_ratio_permille: 0,
                        portal_cluster_count: 0,
                        initial_agents_per_faction: 0,
                        reinforcement_interval_rounds: 0,
                        reinforcement_agents_per_faction: 0,
                        move_budget_cm: 0,
                        portal_operation_range_cm: 0,
                        transfer_range_cm: 0,
                        initial_supply: 0,
                        max_supply: 0,
                        portal_max_hp: 0,
                        attack_damage: 0,
                        recharge_amount: 0,
                        recharge_cost_supply: 0,
                        hack_supply_yield: 0,
                        hack_key_yield_min: 0,
                        hack_key_yield_max: 0,
                        hack_cooldown_rounds: 0,
                        hack_limit_per_epoch: 0,
                        max_link_distance_cm: 0,
                        max_commands_per_agent_round: 0,
                        max_frame_payload_bytes: 0,
                    }
                }
            }

            impl MatchInfo {
                /// Creates a [MatchInfoBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> MatchInfoBuilder<()> {
                    MatchInfoBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_total_rounds: impl ::planus::WriteAsDefault<u32, u32>,
                    field_initial_spawn_window_ms: impl ::planus::WriteAsDefault<u32, u32>,
                    field_decision_window_ms: impl ::planus::WriteAsDefault<u32, u32>,
                    field_priority_bucket_ms: impl ::planus::WriteAsDefault<u32, u32>,
                    field_epoch_length: impl ::planus::WriteAsDefault<u32, u32>,
                    field_map_width_cm: impl ::planus::WriteAsDefault<i32, i32>,
                    field_map_height_cm: impl ::planus::WriteAsDefault<i32, i32>,
                    field_portal_count: impl ::planus::WriteAsDefault<u32, u32>,
                    field_portal_min_distance_cm: impl ::planus::WriteAsDefault<u32, u32>,
                    field_clustered_portal_ratio_permille: impl ::planus::WriteAsDefault<u32, u32>,
                    field_portal_cluster_count: impl ::planus::WriteAsDefault<u32, u32>,
                    field_initial_agents_per_faction: impl ::planus::WriteAsDefault<u32, u32>,
                    field_reinforcement_interval_rounds: impl ::planus::WriteAsDefault<u32, u32>,
                    field_reinforcement_agents_per_faction: impl ::planus::WriteAsDefault<u32, u32>,
                    field_move_budget_cm: impl ::planus::WriteAsDefault<u32, u32>,
                    field_portal_operation_range_cm: impl ::planus::WriteAsDefault<u32, u32>,
                    field_transfer_range_cm: impl ::planus::WriteAsDefault<u32, u32>,
                    field_initial_supply: impl ::planus::WriteAsDefault<u32, u32>,
                    field_max_supply: impl ::planus::WriteAsDefault<u32, u32>,
                    field_portal_max_hp: impl ::planus::WriteAsDefault<u32, u32>,
                    field_attack_damage: impl ::planus::WriteAsDefault<u32, u32>,
                    field_recharge_amount: impl ::planus::WriteAsDefault<u32, u32>,
                    field_recharge_cost_supply: impl ::planus::WriteAsDefault<u32, u32>,
                    field_hack_supply_yield: impl ::planus::WriteAsDefault<u32, u32>,
                    field_hack_key_yield_min: impl ::planus::WriteAsDefault<u32, u32>,
                    field_hack_key_yield_max: impl ::planus::WriteAsDefault<u32, u32>,
                    field_hack_cooldown_rounds: impl ::planus::WriteAsDefault<u32, u32>,
                    field_hack_limit_per_epoch: impl ::planus::WriteAsDefault<u32, u32>,
                    field_max_link_distance_cm: impl ::planus::WriteAsDefault<u32, u32>,
                    field_max_commands_per_agent_round: impl ::planus::WriteAsDefault<u32, u32>,
                    field_max_frame_payload_bytes: impl ::planus::WriteAsDefault<u32, u32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_total_rounds = field_total_rounds.prepare(builder, &0);
                    let prepared_initial_spawn_window_ms =
                        field_initial_spawn_window_ms.prepare(builder, &0);
                    let prepared_decision_window_ms = field_decision_window_ms.prepare(builder, &0);
                    let prepared_priority_bucket_ms = field_priority_bucket_ms.prepare(builder, &0);
                    let prepared_epoch_length = field_epoch_length.prepare(builder, &0);
                    let prepared_map_width_cm = field_map_width_cm.prepare(builder, &0);
                    let prepared_map_height_cm = field_map_height_cm.prepare(builder, &0);
                    let prepared_portal_count = field_portal_count.prepare(builder, &0);
                    let prepared_portal_min_distance_cm =
                        field_portal_min_distance_cm.prepare(builder, &0);
                    let prepared_clustered_portal_ratio_permille =
                        field_clustered_portal_ratio_permille.prepare(builder, &0);
                    let prepared_portal_cluster_count =
                        field_portal_cluster_count.prepare(builder, &0);
                    let prepared_initial_agents_per_faction =
                        field_initial_agents_per_faction.prepare(builder, &0);
                    let prepared_reinforcement_interval_rounds =
                        field_reinforcement_interval_rounds.prepare(builder, &0);
                    let prepared_reinforcement_agents_per_faction =
                        field_reinforcement_agents_per_faction.prepare(builder, &0);
                    let prepared_move_budget_cm = field_move_budget_cm.prepare(builder, &0);
                    let prepared_portal_operation_range_cm =
                        field_portal_operation_range_cm.prepare(builder, &0);
                    let prepared_transfer_range_cm = field_transfer_range_cm.prepare(builder, &0);
                    let prepared_initial_supply = field_initial_supply.prepare(builder, &0);
                    let prepared_max_supply = field_max_supply.prepare(builder, &0);
                    let prepared_portal_max_hp = field_portal_max_hp.prepare(builder, &0);
                    let prepared_attack_damage = field_attack_damage.prepare(builder, &0);
                    let prepared_recharge_amount = field_recharge_amount.prepare(builder, &0);
                    let prepared_recharge_cost_supply =
                        field_recharge_cost_supply.prepare(builder, &0);
                    let prepared_hack_supply_yield = field_hack_supply_yield.prepare(builder, &0);
                    let prepared_hack_key_yield_min = field_hack_key_yield_min.prepare(builder, &0);
                    let prepared_hack_key_yield_max = field_hack_key_yield_max.prepare(builder, &0);
                    let prepared_hack_cooldown_rounds =
                        field_hack_cooldown_rounds.prepare(builder, &0);
                    let prepared_hack_limit_per_epoch =
                        field_hack_limit_per_epoch.prepare(builder, &0);
                    let prepared_max_link_distance_cm =
                        field_max_link_distance_cm.prepare(builder, &0);
                    let prepared_max_commands_per_agent_round =
                        field_max_commands_per_agent_round.prepare(builder, &0);
                    let prepared_max_frame_payload_bytes =
                        field_max_frame_payload_bytes.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<66> =
                        ::core::default::Default::default();
                    if prepared_total_rounds.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }
                    if prepared_initial_spawn_window_ms.is_some() {
                        table_writer.write_entry::<u32>(1);
                    }
                    if prepared_decision_window_ms.is_some() {
                        table_writer.write_entry::<u32>(2);
                    }
                    if prepared_priority_bucket_ms.is_some() {
                        table_writer.write_entry::<u32>(3);
                    }
                    if prepared_epoch_length.is_some() {
                        table_writer.write_entry::<u32>(4);
                    }
                    if prepared_map_width_cm.is_some() {
                        table_writer.write_entry::<i32>(5);
                    }
                    if prepared_map_height_cm.is_some() {
                        table_writer.write_entry::<i32>(6);
                    }
                    if prepared_portal_count.is_some() {
                        table_writer.write_entry::<u32>(7);
                    }
                    if prepared_portal_min_distance_cm.is_some() {
                        table_writer.write_entry::<u32>(8);
                    }
                    if prepared_clustered_portal_ratio_permille.is_some() {
                        table_writer.write_entry::<u32>(9);
                    }
                    if prepared_portal_cluster_count.is_some() {
                        table_writer.write_entry::<u32>(10);
                    }
                    if prepared_initial_agents_per_faction.is_some() {
                        table_writer.write_entry::<u32>(11);
                    }
                    if prepared_reinforcement_interval_rounds.is_some() {
                        table_writer.write_entry::<u32>(12);
                    }
                    if prepared_reinforcement_agents_per_faction.is_some() {
                        table_writer.write_entry::<u32>(13);
                    }
                    if prepared_move_budget_cm.is_some() {
                        table_writer.write_entry::<u32>(14);
                    }
                    if prepared_portal_operation_range_cm.is_some() {
                        table_writer.write_entry::<u32>(15);
                    }
                    if prepared_transfer_range_cm.is_some() {
                        table_writer.write_entry::<u32>(16);
                    }
                    if prepared_initial_supply.is_some() {
                        table_writer.write_entry::<u32>(17);
                    }
                    if prepared_max_supply.is_some() {
                        table_writer.write_entry::<u32>(18);
                    }
                    if prepared_portal_max_hp.is_some() {
                        table_writer.write_entry::<u32>(19);
                    }
                    if prepared_attack_damage.is_some() {
                        table_writer.write_entry::<u32>(20);
                    }
                    if prepared_recharge_amount.is_some() {
                        table_writer.write_entry::<u32>(21);
                    }
                    if prepared_recharge_cost_supply.is_some() {
                        table_writer.write_entry::<u32>(22);
                    }
                    if prepared_hack_supply_yield.is_some() {
                        table_writer.write_entry::<u32>(23);
                    }
                    if prepared_hack_key_yield_min.is_some() {
                        table_writer.write_entry::<u32>(24);
                    }
                    if prepared_hack_key_yield_max.is_some() {
                        table_writer.write_entry::<u32>(25);
                    }
                    if prepared_hack_cooldown_rounds.is_some() {
                        table_writer.write_entry::<u32>(26);
                    }
                    if prepared_hack_limit_per_epoch.is_some() {
                        table_writer.write_entry::<u32>(27);
                    }
                    if prepared_max_link_distance_cm.is_some() {
                        table_writer.write_entry::<u32>(28);
                    }
                    if prepared_max_commands_per_agent_round.is_some() {
                        table_writer.write_entry::<u32>(29);
                    }
                    if prepared_max_frame_payload_bytes.is_some() {
                        table_writer.write_entry::<u32>(30);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_total_rounds) =
                                prepared_total_rounds
                            {
                                object_writer.write::<_, _, 4>(&prepared_total_rounds);
                            }
                            if let ::core::option::Option::Some(prepared_initial_spawn_window_ms) =
                                prepared_initial_spawn_window_ms
                            {
                                object_writer.write::<_, _, 4>(&prepared_initial_spawn_window_ms);
                            }
                            if let ::core::option::Option::Some(prepared_decision_window_ms) =
                                prepared_decision_window_ms
                            {
                                object_writer.write::<_, _, 4>(&prepared_decision_window_ms);
                            }
                            if let ::core::option::Option::Some(prepared_priority_bucket_ms) =
                                prepared_priority_bucket_ms
                            {
                                object_writer.write::<_, _, 4>(&prepared_priority_bucket_ms);
                            }
                            if let ::core::option::Option::Some(prepared_epoch_length) =
                                prepared_epoch_length
                            {
                                object_writer.write::<_, _, 4>(&prepared_epoch_length);
                            }
                            if let ::core::option::Option::Some(prepared_map_width_cm) =
                                prepared_map_width_cm
                            {
                                object_writer.write::<_, _, 4>(&prepared_map_width_cm);
                            }
                            if let ::core::option::Option::Some(prepared_map_height_cm) =
                                prepared_map_height_cm
                            {
                                object_writer.write::<_, _, 4>(&prepared_map_height_cm);
                            }
                            if let ::core::option::Option::Some(prepared_portal_count) =
                                prepared_portal_count
                            {
                                object_writer.write::<_, _, 4>(&prepared_portal_count);
                            }
                            if let ::core::option::Option::Some(prepared_portal_min_distance_cm) =
                                prepared_portal_min_distance_cm
                            {
                                object_writer.write::<_, _, 4>(&prepared_portal_min_distance_cm);
                            }
                            if let ::core::option::Option::Some(
                                prepared_clustered_portal_ratio_permille,
                            ) = prepared_clustered_portal_ratio_permille
                            {
                                object_writer
                                    .write::<_, _, 4>(&prepared_clustered_portal_ratio_permille);
                            }
                            if let ::core::option::Option::Some(prepared_portal_cluster_count) =
                                prepared_portal_cluster_count
                            {
                                object_writer.write::<_, _, 4>(&prepared_portal_cluster_count);
                            }
                            if let ::core::option::Option::Some(
                                prepared_initial_agents_per_faction,
                            ) = prepared_initial_agents_per_faction
                            {
                                object_writer
                                    .write::<_, _, 4>(&prepared_initial_agents_per_faction);
                            }
                            if let ::core::option::Option::Some(
                                prepared_reinforcement_interval_rounds,
                            ) = prepared_reinforcement_interval_rounds
                            {
                                object_writer
                                    .write::<_, _, 4>(&prepared_reinforcement_interval_rounds);
                            }
                            if let ::core::option::Option::Some(
                                prepared_reinforcement_agents_per_faction,
                            ) = prepared_reinforcement_agents_per_faction
                            {
                                object_writer
                                    .write::<_, _, 4>(&prepared_reinforcement_agents_per_faction);
                            }
                            if let ::core::option::Option::Some(prepared_move_budget_cm) =
                                prepared_move_budget_cm
                            {
                                object_writer.write::<_, _, 4>(&prepared_move_budget_cm);
                            }
                            if let ::core::option::Option::Some(
                                prepared_portal_operation_range_cm,
                            ) = prepared_portal_operation_range_cm
                            {
                                object_writer.write::<_, _, 4>(&prepared_portal_operation_range_cm);
                            }
                            if let ::core::option::Option::Some(prepared_transfer_range_cm) =
                                prepared_transfer_range_cm
                            {
                                object_writer.write::<_, _, 4>(&prepared_transfer_range_cm);
                            }
                            if let ::core::option::Option::Some(prepared_initial_supply) =
                                prepared_initial_supply
                            {
                                object_writer.write::<_, _, 4>(&prepared_initial_supply);
                            }
                            if let ::core::option::Option::Some(prepared_max_supply) =
                                prepared_max_supply
                            {
                                object_writer.write::<_, _, 4>(&prepared_max_supply);
                            }
                            if let ::core::option::Option::Some(prepared_portal_max_hp) =
                                prepared_portal_max_hp
                            {
                                object_writer.write::<_, _, 4>(&prepared_portal_max_hp);
                            }
                            if let ::core::option::Option::Some(prepared_attack_damage) =
                                prepared_attack_damage
                            {
                                object_writer.write::<_, _, 4>(&prepared_attack_damage);
                            }
                            if let ::core::option::Option::Some(prepared_recharge_amount) =
                                prepared_recharge_amount
                            {
                                object_writer.write::<_, _, 4>(&prepared_recharge_amount);
                            }
                            if let ::core::option::Option::Some(prepared_recharge_cost_supply) =
                                prepared_recharge_cost_supply
                            {
                                object_writer.write::<_, _, 4>(&prepared_recharge_cost_supply);
                            }
                            if let ::core::option::Option::Some(prepared_hack_supply_yield) =
                                prepared_hack_supply_yield
                            {
                                object_writer.write::<_, _, 4>(&prepared_hack_supply_yield);
                            }
                            if let ::core::option::Option::Some(prepared_hack_key_yield_min) =
                                prepared_hack_key_yield_min
                            {
                                object_writer.write::<_, _, 4>(&prepared_hack_key_yield_min);
                            }
                            if let ::core::option::Option::Some(prepared_hack_key_yield_max) =
                                prepared_hack_key_yield_max
                            {
                                object_writer.write::<_, _, 4>(&prepared_hack_key_yield_max);
                            }
                            if let ::core::option::Option::Some(prepared_hack_cooldown_rounds) =
                                prepared_hack_cooldown_rounds
                            {
                                object_writer.write::<_, _, 4>(&prepared_hack_cooldown_rounds);
                            }
                            if let ::core::option::Option::Some(prepared_hack_limit_per_epoch) =
                                prepared_hack_limit_per_epoch
                            {
                                object_writer.write::<_, _, 4>(&prepared_hack_limit_per_epoch);
                            }
                            if let ::core::option::Option::Some(prepared_max_link_distance_cm) =
                                prepared_max_link_distance_cm
                            {
                                object_writer.write::<_, _, 4>(&prepared_max_link_distance_cm);
                            }
                            if let ::core::option::Option::Some(
                                prepared_max_commands_per_agent_round,
                            ) = prepared_max_commands_per_agent_round
                            {
                                object_writer
                                    .write::<_, _, 4>(&prepared_max_commands_per_agent_round);
                            }
                            if let ::core::option::Option::Some(prepared_max_frame_payload_bytes) =
                                prepared_max_frame_payload_bytes
                            {
                                object_writer.write::<_, _, 4>(&prepared_max_frame_payload_bytes);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<MatchInfo>> for MatchInfo {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<MatchInfo> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<MatchInfo>> for MatchInfo {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<MatchInfo>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<MatchInfo> for MatchInfo {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<MatchInfo> {
                    MatchInfo::create(
                        builder,
                        self.total_rounds,
                        self.initial_spawn_window_ms,
                        self.decision_window_ms,
                        self.priority_bucket_ms,
                        self.epoch_length,
                        self.map_width_cm,
                        self.map_height_cm,
                        self.portal_count,
                        self.portal_min_distance_cm,
                        self.clustered_portal_ratio_permille,
                        self.portal_cluster_count,
                        self.initial_agents_per_faction,
                        self.reinforcement_interval_rounds,
                        self.reinforcement_agents_per_faction,
                        self.move_budget_cm,
                        self.portal_operation_range_cm,
                        self.transfer_range_cm,
                        self.initial_supply,
                        self.max_supply,
                        self.portal_max_hp,
                        self.attack_damage,
                        self.recharge_amount,
                        self.recharge_cost_supply,
                        self.hack_supply_yield,
                        self.hack_key_yield_min,
                        self.hack_key_yield_max,
                        self.hack_cooldown_rounds,
                        self.hack_limit_per_epoch,
                        self.max_link_distance_cm,
                        self.max_commands_per_agent_round,
                        self.max_frame_payload_bytes,
                    )
                }
            }

            /// Builder for serializing an instance of the [MatchInfo] type.
            ///
            /// Can be created using the [MatchInfo::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct MatchInfoBuilder<State>(State);

            impl MatchInfoBuilder<()> {
                /// Setter for the [`total_rounds` field](MatchInfo#structfield.total_rounds).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn total_rounds<T0>(self, value: T0) -> MatchInfoBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    MatchInfoBuilder((value,))
                }

                /// Sets the [`total_rounds` field](MatchInfo#structfield.total_rounds) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn total_rounds_as_default(
                    self,
                ) -> MatchInfoBuilder<(::planus::DefaultValue,)> {
                    self.total_rounds(::planus::DefaultValue)
                }
            }

            impl<T0> MatchInfoBuilder<(T0,)> {
                /// Setter for the [`initial_spawn_window_ms` field](MatchInfo#structfield.initial_spawn_window_ms).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn initial_spawn_window_ms<T1>(self, value: T1) -> MatchInfoBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0,) = self.0;
                    MatchInfoBuilder((v0, value))
                }

                /// Sets the [`initial_spawn_window_ms` field](MatchInfo#structfield.initial_spawn_window_ms) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn initial_spawn_window_ms_as_default(
                    self,
                ) -> MatchInfoBuilder<(T0, ::planus::DefaultValue)> {
                    self.initial_spawn_window_ms(::planus::DefaultValue)
                }
            }

            impl<T0, T1> MatchInfoBuilder<(T0, T1)> {
                /// Setter for the [`decision_window_ms` field](MatchInfo#structfield.decision_window_ms).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn decision_window_ms<T2>(self, value: T2) -> MatchInfoBuilder<(T0, T1, T2)>
                where
                    T2: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1) = self.0;
                    MatchInfoBuilder((v0, v1, value))
                }

                /// Sets the [`decision_window_ms` field](MatchInfo#structfield.decision_window_ms) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn decision_window_ms_as_default(
                    self,
                ) -> MatchInfoBuilder<(T0, T1, ::planus::DefaultValue)> {
                    self.decision_window_ms(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2> MatchInfoBuilder<(T0, T1, T2)> {
                /// Setter for the [`priority_bucket_ms` field](MatchInfo#structfield.priority_bucket_ms).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn priority_bucket_ms<T3>(self, value: T3) -> MatchInfoBuilder<(T0, T1, T2, T3)>
                where
                    T3: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2) = self.0;
                    MatchInfoBuilder((v0, v1, v2, value))
                }

                /// Sets the [`priority_bucket_ms` field](MatchInfo#structfield.priority_bucket_ms) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn priority_bucket_ms_as_default(
                    self,
                ) -> MatchInfoBuilder<(T0, T1, T2, ::planus::DefaultValue)> {
                    self.priority_bucket_ms(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3> MatchInfoBuilder<(T0, T1, T2, T3)> {
                /// Setter for the [`epoch_length` field](MatchInfo#structfield.epoch_length).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn epoch_length<T4>(self, value: T4) -> MatchInfoBuilder<(T0, T1, T2, T3, T4)>
                where
                    T4: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, value))
                }

                /// Sets the [`epoch_length` field](MatchInfo#structfield.epoch_length) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn epoch_length_as_default(
                    self,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)> {
                    self.epoch_length(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4> MatchInfoBuilder<(T0, T1, T2, T3, T4)> {
                /// Setter for the [`map_width_cm` field](MatchInfo#structfield.map_width_cm).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn map_width_cm<T5>(
                    self,
                    value: T5,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5)>
                where
                    T5: ::planus::WriteAsDefault<i32, i32>,
                {
                    let (v0, v1, v2, v3, v4) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, v4, value))
                }

                /// Sets the [`map_width_cm` field](MatchInfo#structfield.map_width_cm) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn map_width_cm_as_default(
                    self,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, ::planus::DefaultValue)>
                {
                    self.map_width_cm(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5)> {
                /// Setter for the [`map_height_cm` field](MatchInfo#structfield.map_height_cm).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn map_height_cm<T6>(
                    self,
                    value: T6,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6)>
                where
                    T6: ::planus::WriteAsDefault<i32, i32>,
                {
                    let (v0, v1, v2, v3, v4, v5) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, v4, v5, value))
                }

                /// Sets the [`map_height_cm` field](MatchInfo#structfield.map_height_cm) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn map_height_cm_as_default(
                    self,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, ::planus::DefaultValue)>
                {
                    self.map_height_cm(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6)> {
                /// Setter for the [`portal_count` field](MatchInfo#structfield.portal_count).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_count<T7>(
                    self,
                    value: T7,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)>
                where
                    T7: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, v4, v5, v6, value))
                }

                /// Sets the [`portal_count` field](MatchInfo#structfield.portal_count) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_count_as_default(
                    self,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, ::planus::DefaultValue)>
                {
                    self.portal_count(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)> {
                /// Setter for the [`portal_min_distance_cm` field](MatchInfo#structfield.portal_min_distance_cm).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_min_distance_cm<T8>(
                    self,
                    value: T8,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)>
                where
                    T8: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, v4, v5, v6, v7, value))
                }

                /// Sets the [`portal_min_distance_cm` field](MatchInfo#structfield.portal_min_distance_cm) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_min_distance_cm_as_default(
                    self,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, ::planus::DefaultValue)>
                {
                    self.portal_min_distance_cm(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)> {
                /// Setter for the [`clustered_portal_ratio_permille` field](MatchInfo#structfield.clustered_portal_ratio_permille).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn clustered_portal_ratio_permille<T9>(
                    self,
                    value: T9,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
                where
                    T9: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, value))
                }

                /// Sets the [`clustered_portal_ratio_permille` field](MatchInfo#structfield.clustered_portal_ratio_permille) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn clustered_portal_ratio_permille_as_default(
                    self,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, ::planus::DefaultValue)>
                {
                    self.clustered_portal_ratio_permille(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9>
                MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
            {
                /// Setter for the [`portal_cluster_count` field](MatchInfo#structfield.portal_cluster_count).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_cluster_count<T10>(
                    self,
                    value: T10,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)>
                where
                    T10: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, value))
                }

                /// Sets the [`portal_cluster_count` field](MatchInfo#structfield.portal_cluster_count) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_cluster_count_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    ::planus::DefaultValue,
                )> {
                    self.portal_cluster_count(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10>
                MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)>
            {
                /// Setter for the [`initial_agents_per_faction` field](MatchInfo#structfield.initial_agents_per_faction).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn initial_agents_per_faction<T11>(
                    self,
                    value: T11,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)>
                where
                    T11: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, value))
                }

                /// Sets the [`initial_agents_per_faction` field](MatchInfo#structfield.initial_agents_per_faction) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn initial_agents_per_faction_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    ::planus::DefaultValue,
                )> {
                    self.initial_agents_per_faction(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11>
                MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)>
            {
                /// Setter for the [`reinforcement_interval_rounds` field](MatchInfo#structfield.reinforcement_interval_rounds).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn reinforcement_interval_rounds<T12>(
                    self,
                    value: T12,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
                where
                    T12: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, value))
                }

                /// Sets the [`reinforcement_interval_rounds` field](MatchInfo#structfield.reinforcement_interval_rounds) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn reinforcement_interval_rounds_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    ::planus::DefaultValue,
                )> {
                    self.reinforcement_interval_rounds(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12>
                MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
            {
                /// Setter for the [`reinforcement_agents_per_faction` field](MatchInfo#structfield.reinforcement_agents_per_faction).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn reinforcement_agents_per_faction<T13>(
                    self,
                    value: T13,
                ) -> MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13)>
                where
                    T13: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12) = self.0;
                    MatchInfoBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, value))
                }

                /// Sets the [`reinforcement_agents_per_faction` field](MatchInfo#structfield.reinforcement_agents_per_faction) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn reinforcement_agents_per_faction_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    ::planus::DefaultValue,
                )> {
                    self.reinforcement_agents_per_faction(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13>
                MatchInfoBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13)>
            {
                /// Setter for the [`move_budget_cm` field](MatchInfo#structfield.move_budget_cm).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn move_budget_cm<T14>(
                    self,
                    value: T14,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                )>
                where
                    T14: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, value,
                    ))
                }

                /// Sets the [`move_budget_cm` field](MatchInfo#structfield.move_budget_cm) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn move_budget_cm_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    ::planus::DefaultValue,
                )> {
                    self.move_budget_cm(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14>
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                )>
            {
                /// Setter for the [`portal_operation_range_cm` field](MatchInfo#structfield.portal_operation_range_cm).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_operation_range_cm<T15>(
                    self,
                    value: T15,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                )>
                where
                    T15: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, value,
                    ))
                }

                /// Sets the [`portal_operation_range_cm` field](MatchInfo#structfield.portal_operation_range_cm) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_operation_range_cm_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    ::planus::DefaultValue,
                )> {
                    self.portal_operation_range_cm(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15>
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                )>
            {
                /// Setter for the [`transfer_range_cm` field](MatchInfo#structfield.transfer_range_cm).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn transfer_range_cm<T16>(
                    self,
                    value: T16,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                )>
                where
                    T16: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15) =
                        self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, value,
                    ))
                }

                /// Sets the [`transfer_range_cm` field](MatchInfo#structfield.transfer_range_cm) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn transfer_range_cm_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    ::planus::DefaultValue,
                )> {
                    self.transfer_range_cm(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16>
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                )>
            {
                /// Setter for the [`initial_supply` field](MatchInfo#structfield.initial_supply).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn initial_supply<T17>(
                    self,
                    value: T17,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                )>
                where
                    T17: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16) =
                        self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        value,
                    ))
                }

                /// Sets the [`initial_supply` field](MatchInfo#structfield.initial_supply) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn initial_supply_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    ::planus::DefaultValue,
                )> {
                    self.initial_supply(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17>
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                )>
            {
                /// Setter for the [`max_supply` field](MatchInfo#structfield.max_supply).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn max_supply<T18>(
                    self,
                    value: T18,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                )>
                where
                    T18: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, value,
                    ))
                }

                /// Sets the [`max_supply` field](MatchInfo#structfield.max_supply) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn max_supply_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    ::planus::DefaultValue,
                )> {
                    self.max_supply(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                )>
            {
                /// Setter for the [`portal_max_hp` field](MatchInfo#structfield.portal_max_hp).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_max_hp<T19>(
                    self,
                    value: T19,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                )>
                where
                    T19: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, value,
                    ))
                }

                /// Sets the [`portal_max_hp` field](MatchInfo#structfield.portal_max_hp) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_max_hp_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    ::planus::DefaultValue,
                )> {
                    self.portal_max_hp(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                )>
            {
                /// Setter for the [`attack_damage` field](MatchInfo#structfield.attack_damage).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn attack_damage<T20>(
                    self,
                    value: T20,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                )>
                where
                    T20: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, value,
                    ))
                }

                /// Sets the [`attack_damage` field](MatchInfo#structfield.attack_damage) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn attack_damage_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    ::planus::DefaultValue,
                )> {
                    self.attack_damage(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                )>
            {
                /// Setter for the [`recharge_amount` field](MatchInfo#structfield.recharge_amount).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn recharge_amount<T21>(
                    self,
                    value: T21,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                )>
                where
                    T21: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, value,
                    ))
                }

                /// Sets the [`recharge_amount` field](MatchInfo#structfield.recharge_amount) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn recharge_amount_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    ::planus::DefaultValue,
                )> {
                    self.recharge_amount(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                )>
            {
                /// Setter for the [`recharge_cost_supply` field](MatchInfo#structfield.recharge_cost_supply).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn recharge_cost_supply<T22>(
                    self,
                    value: T22,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                )>
                where
                    T22: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, v21, value,
                    ))
                }

                /// Sets the [`recharge_cost_supply` field](MatchInfo#structfield.recharge_cost_supply) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn recharge_cost_supply_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    ::planus::DefaultValue,
                )> {
                    self.recharge_cost_supply(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
                T22,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                )>
            {
                /// Setter for the [`hack_supply_yield` field](MatchInfo#structfield.hack_supply_yield).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_supply_yield<T23>(
                    self,
                    value: T23,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                )>
                where
                    T23: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                        v22,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, v21, v22, value,
                    ))
                }

                /// Sets the [`hack_supply_yield` field](MatchInfo#structfield.hack_supply_yield) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_supply_yield_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    ::planus::DefaultValue,
                )> {
                    self.hack_supply_yield(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
                T22,
                T23,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                )>
            {
                /// Setter for the [`hack_key_yield_min` field](MatchInfo#structfield.hack_key_yield_min).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_key_yield_min<T24>(
                    self,
                    value: T24,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                )>
                where
                    T24: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                        v22,
                        v23,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, v21, v22, v23, value,
                    ))
                }

                /// Sets the [`hack_key_yield_min` field](MatchInfo#structfield.hack_key_yield_min) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_key_yield_min_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    ::planus::DefaultValue,
                )> {
                    self.hack_key_yield_min(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
                T22,
                T23,
                T24,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                )>
            {
                /// Setter for the [`hack_key_yield_max` field](MatchInfo#structfield.hack_key_yield_max).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_key_yield_max<T25>(
                    self,
                    value: T25,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                )>
                where
                    T25: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                        v22,
                        v23,
                        v24,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, v21, v22, v23, v24, value,
                    ))
                }

                /// Sets the [`hack_key_yield_max` field](MatchInfo#structfield.hack_key_yield_max) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_key_yield_max_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    ::planus::DefaultValue,
                )> {
                    self.hack_key_yield_max(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
                T22,
                T23,
                T24,
                T25,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                )>
            {
                /// Setter for the [`hack_cooldown_rounds` field](MatchInfo#structfield.hack_cooldown_rounds).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_cooldown_rounds<T26>(
                    self,
                    value: T26,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                )>
                where
                    T26: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                        v22,
                        v23,
                        v24,
                        v25,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, v21, v22, v23, v24, v25, value,
                    ))
                }

                /// Sets the [`hack_cooldown_rounds` field](MatchInfo#structfield.hack_cooldown_rounds) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_cooldown_rounds_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    ::planus::DefaultValue,
                )> {
                    self.hack_cooldown_rounds(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
                T22,
                T23,
                T24,
                T25,
                T26,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                )>
            {
                /// Setter for the [`hack_limit_per_epoch` field](MatchInfo#structfield.hack_limit_per_epoch).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_limit_per_epoch<T27>(
                    self,
                    value: T27,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                )>
                where
                    T27: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                        v22,
                        v23,
                        v24,
                        v25,
                        v26,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, v21, v22, v23, v24, v25, v26, value,
                    ))
                }

                /// Sets the [`hack_limit_per_epoch` field](MatchInfo#structfield.hack_limit_per_epoch) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn hack_limit_per_epoch_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    ::planus::DefaultValue,
                )> {
                    self.hack_limit_per_epoch(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
                T22,
                T23,
                T24,
                T25,
                T26,
                T27,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                )>
            {
                /// Setter for the [`max_link_distance_cm` field](MatchInfo#structfield.max_link_distance_cm).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn max_link_distance_cm<T28>(
                    self,
                    value: T28,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                )>
                where
                    T28: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                        v22,
                        v23,
                        v24,
                        v25,
                        v26,
                        v27,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, v21, v22, v23, v24, v25, v26, v27, value,
                    ))
                }

                /// Sets the [`max_link_distance_cm` field](MatchInfo#structfield.max_link_distance_cm) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn max_link_distance_cm_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    ::planus::DefaultValue,
                )> {
                    self.max_link_distance_cm(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
                T22,
                T23,
                T24,
                T25,
                T26,
                T27,
                T28,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                )>
            {
                /// Setter for the [`max_commands_per_agent_round` field](MatchInfo#structfield.max_commands_per_agent_round).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn max_commands_per_agent_round<T29>(
                    self,
                    value: T29,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                    T29,
                )>
                where
                    T29: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                        v22,
                        v23,
                        v24,
                        v25,
                        v26,
                        v27,
                        v28,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, v21, v22, v23, v24, v25, v26, v27, v28, value,
                    ))
                }

                /// Sets the [`max_commands_per_agent_round` field](MatchInfo#structfield.max_commands_per_agent_round) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn max_commands_per_agent_round_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                    ::planus::DefaultValue,
                )> {
                    self.max_commands_per_agent_round(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
                T22,
                T23,
                T24,
                T25,
                T26,
                T27,
                T28,
                T29,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                    T29,
                )>
            {
                /// Setter for the [`max_frame_payload_bytes` field](MatchInfo#structfield.max_frame_payload_bytes).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn max_frame_payload_bytes<T30>(
                    self,
                    value: T30,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                    T29,
                    T30,
                )>
                where
                    T30: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                        v22,
                        v23,
                        v24,
                        v25,
                        v26,
                        v27,
                        v28,
                        v29,
                    ) = self.0;
                    MatchInfoBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, v19, v20, v21, v22, v23, v24, v25, v26, v27, v28, v29, value,
                    ))
                }

                /// Sets the [`max_frame_payload_bytes` field](MatchInfo#structfield.max_frame_payload_bytes) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn max_frame_payload_bytes_as_default(
                    self,
                ) -> MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                    T29,
                    ::planus::DefaultValue,
                )> {
                    self.max_frame_payload_bytes(::planus::DefaultValue)
                }
            }

            impl<
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
                T13,
                T14,
                T15,
                T16,
                T17,
                T18,
                T19,
                T20,
                T21,
                T22,
                T23,
                T24,
                T25,
                T26,
                T27,
                T28,
                T29,
                T30,
            >
                MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                    T29,
                    T30,
                )>
            {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [MatchInfo].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<MatchInfo>
                where
                    Self: ::planus::WriteAsOffset<MatchInfo>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsDefault<u32, u32>,
                T3: ::planus::WriteAsDefault<u32, u32>,
                T4: ::planus::WriteAsDefault<u32, u32>,
                T5: ::planus::WriteAsDefault<i32, i32>,
                T6: ::planus::WriteAsDefault<i32, i32>,
                T7: ::planus::WriteAsDefault<u32, u32>,
                T8: ::planus::WriteAsDefault<u32, u32>,
                T9: ::planus::WriteAsDefault<u32, u32>,
                T10: ::planus::WriteAsDefault<u32, u32>,
                T11: ::planus::WriteAsDefault<u32, u32>,
                T12: ::planus::WriteAsDefault<u32, u32>,
                T13: ::planus::WriteAsDefault<u32, u32>,
                T14: ::planus::WriteAsDefault<u32, u32>,
                T15: ::planus::WriteAsDefault<u32, u32>,
                T16: ::planus::WriteAsDefault<u32, u32>,
                T17: ::planus::WriteAsDefault<u32, u32>,
                T18: ::planus::WriteAsDefault<u32, u32>,
                T19: ::planus::WriteAsDefault<u32, u32>,
                T20: ::planus::WriteAsDefault<u32, u32>,
                T21: ::planus::WriteAsDefault<u32, u32>,
                T22: ::planus::WriteAsDefault<u32, u32>,
                T23: ::planus::WriteAsDefault<u32, u32>,
                T24: ::planus::WriteAsDefault<u32, u32>,
                T25: ::planus::WriteAsDefault<u32, u32>,
                T26: ::planus::WriteAsDefault<u32, u32>,
                T27: ::planus::WriteAsDefault<u32, u32>,
                T28: ::planus::WriteAsDefault<u32, u32>,
                T29: ::planus::WriteAsDefault<u32, u32>,
                T30: ::planus::WriteAsDefault<u32, u32>,
            > ::planus::WriteAs<::planus::Offset<MatchInfo>>
                for MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                    T29,
                    T30,
                )>
            {
                type Prepared = ::planus::Offset<MatchInfo>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<MatchInfo> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsDefault<u32, u32>,
                T3: ::planus::WriteAsDefault<u32, u32>,
                T4: ::planus::WriteAsDefault<u32, u32>,
                T5: ::planus::WriteAsDefault<i32, i32>,
                T6: ::planus::WriteAsDefault<i32, i32>,
                T7: ::planus::WriteAsDefault<u32, u32>,
                T8: ::planus::WriteAsDefault<u32, u32>,
                T9: ::planus::WriteAsDefault<u32, u32>,
                T10: ::planus::WriteAsDefault<u32, u32>,
                T11: ::planus::WriteAsDefault<u32, u32>,
                T12: ::planus::WriteAsDefault<u32, u32>,
                T13: ::planus::WriteAsDefault<u32, u32>,
                T14: ::planus::WriteAsDefault<u32, u32>,
                T15: ::planus::WriteAsDefault<u32, u32>,
                T16: ::planus::WriteAsDefault<u32, u32>,
                T17: ::planus::WriteAsDefault<u32, u32>,
                T18: ::planus::WriteAsDefault<u32, u32>,
                T19: ::planus::WriteAsDefault<u32, u32>,
                T20: ::planus::WriteAsDefault<u32, u32>,
                T21: ::planus::WriteAsDefault<u32, u32>,
                T22: ::planus::WriteAsDefault<u32, u32>,
                T23: ::planus::WriteAsDefault<u32, u32>,
                T24: ::planus::WriteAsDefault<u32, u32>,
                T25: ::planus::WriteAsDefault<u32, u32>,
                T26: ::planus::WriteAsDefault<u32, u32>,
                T27: ::planus::WriteAsDefault<u32, u32>,
                T28: ::planus::WriteAsDefault<u32, u32>,
                T29: ::planus::WriteAsDefault<u32, u32>,
                T30: ::planus::WriteAsDefault<u32, u32>,
            > ::planus::WriteAsOptional<::planus::Offset<MatchInfo>>
                for MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                    T29,
                    T30,
                )>
            {
                type Prepared = ::planus::Offset<MatchInfo>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<MatchInfo>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsDefault<u32, u32>,
                T3: ::planus::WriteAsDefault<u32, u32>,
                T4: ::planus::WriteAsDefault<u32, u32>,
                T5: ::planus::WriteAsDefault<i32, i32>,
                T6: ::planus::WriteAsDefault<i32, i32>,
                T7: ::planus::WriteAsDefault<u32, u32>,
                T8: ::planus::WriteAsDefault<u32, u32>,
                T9: ::planus::WriteAsDefault<u32, u32>,
                T10: ::planus::WriteAsDefault<u32, u32>,
                T11: ::planus::WriteAsDefault<u32, u32>,
                T12: ::planus::WriteAsDefault<u32, u32>,
                T13: ::planus::WriteAsDefault<u32, u32>,
                T14: ::planus::WriteAsDefault<u32, u32>,
                T15: ::planus::WriteAsDefault<u32, u32>,
                T16: ::planus::WriteAsDefault<u32, u32>,
                T17: ::planus::WriteAsDefault<u32, u32>,
                T18: ::planus::WriteAsDefault<u32, u32>,
                T19: ::planus::WriteAsDefault<u32, u32>,
                T20: ::planus::WriteAsDefault<u32, u32>,
                T21: ::planus::WriteAsDefault<u32, u32>,
                T22: ::planus::WriteAsDefault<u32, u32>,
                T23: ::planus::WriteAsDefault<u32, u32>,
                T24: ::planus::WriteAsDefault<u32, u32>,
                T25: ::planus::WriteAsDefault<u32, u32>,
                T26: ::planus::WriteAsDefault<u32, u32>,
                T27: ::planus::WriteAsDefault<u32, u32>,
                T28: ::planus::WriteAsDefault<u32, u32>,
                T29: ::planus::WriteAsDefault<u32, u32>,
                T30: ::planus::WriteAsDefault<u32, u32>,
            > ::planus::WriteAsOffset<MatchInfo>
                for MatchInfoBuilder<(
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
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                    T20,
                    T21,
                    T22,
                    T23,
                    T24,
                    T25,
                    T26,
                    T27,
                    T28,
                    T29,
                    T30,
                )>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<MatchInfo> {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                        v20,
                        v21,
                        v22,
                        v23,
                        v24,
                        v25,
                        v26,
                        v27,
                        v28,
                        v29,
                        v30,
                    ) = &self.0;
                    MatchInfo::create(
                        builder, v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14,
                        v15, v16, v17, v18, v19, v20, v21, v22, v23, v24, v25, v26, v27, v28, v29,
                        v30,
                    )
                }
            }

            /// Reference to a deserialized [MatchInfo].
            #[derive(Copy, Clone)]
            pub struct MatchInfoRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> MatchInfoRef<'a> {
                /// Getter for the [`total_rounds` field](MatchInfo#structfield.total_rounds).
                #[inline]
                pub fn total_rounds(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "MatchInfo", "total_rounds")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`initial_spawn_window_ms` field](MatchInfo#structfield.initial_spawn_window_ms).
                #[inline]
                pub fn initial_spawn_window_ms(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(1, "MatchInfo", "initial_spawn_window_ms")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`decision_window_ms` field](MatchInfo#structfield.decision_window_ms).
                #[inline]
                pub fn decision_window_ms(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(2, "MatchInfo", "decision_window_ms")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`priority_bucket_ms` field](MatchInfo#structfield.priority_bucket_ms).
                #[inline]
                pub fn priority_bucket_ms(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(3, "MatchInfo", "priority_bucket_ms")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`epoch_length` field](MatchInfo#structfield.epoch_length).
                #[inline]
                pub fn epoch_length(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(4, "MatchInfo", "epoch_length")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`map_width_cm` field](MatchInfo#structfield.map_width_cm).
                #[inline]
                pub fn map_width_cm(&self) -> ::planus::Result<i32> {
                    ::core::result::Result::Ok(
                        self.0.access(5, "MatchInfo", "map_width_cm")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`map_height_cm` field](MatchInfo#structfield.map_height_cm).
                #[inline]
                pub fn map_height_cm(&self) -> ::planus::Result<i32> {
                    ::core::result::Result::Ok(
                        self.0.access(6, "MatchInfo", "map_height_cm")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`portal_count` field](MatchInfo#structfield.portal_count).
                #[inline]
                pub fn portal_count(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(7, "MatchInfo", "portal_count")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`portal_min_distance_cm` field](MatchInfo#structfield.portal_min_distance_cm).
                #[inline]
                pub fn portal_min_distance_cm(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(8, "MatchInfo", "portal_min_distance_cm")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`clustered_portal_ratio_permille` field](MatchInfo#structfield.clustered_portal_ratio_permille).
                #[inline]
                pub fn clustered_portal_ratio_permille(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(9, "MatchInfo", "clustered_portal_ratio_permille")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`portal_cluster_count` field](MatchInfo#structfield.portal_cluster_count).
                #[inline]
                pub fn portal_cluster_count(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(10, "MatchInfo", "portal_cluster_count")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`initial_agents_per_faction` field](MatchInfo#structfield.initial_agents_per_faction).
                #[inline]
                pub fn initial_agents_per_faction(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(11, "MatchInfo", "initial_agents_per_faction")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`reinforcement_interval_rounds` field](MatchInfo#structfield.reinforcement_interval_rounds).
                #[inline]
                pub fn reinforcement_interval_rounds(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(12, "MatchInfo", "reinforcement_interval_rounds")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`reinforcement_agents_per_faction` field](MatchInfo#structfield.reinforcement_agents_per_faction).
                #[inline]
                pub fn reinforcement_agents_per_faction(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(13, "MatchInfo", "reinforcement_agents_per_faction")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`move_budget_cm` field](MatchInfo#structfield.move_budget_cm).
                #[inline]
                pub fn move_budget_cm(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(14, "MatchInfo", "move_budget_cm")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`portal_operation_range_cm` field](MatchInfo#structfield.portal_operation_range_cm).
                #[inline]
                pub fn portal_operation_range_cm(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(15, "MatchInfo", "portal_operation_range_cm")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`transfer_range_cm` field](MatchInfo#structfield.transfer_range_cm).
                #[inline]
                pub fn transfer_range_cm(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(16, "MatchInfo", "transfer_range_cm")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`initial_supply` field](MatchInfo#structfield.initial_supply).
                #[inline]
                pub fn initial_supply(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(17, "MatchInfo", "initial_supply")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`max_supply` field](MatchInfo#structfield.max_supply).
                #[inline]
                pub fn max_supply(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(18, "MatchInfo", "max_supply")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`portal_max_hp` field](MatchInfo#structfield.portal_max_hp).
                #[inline]
                pub fn portal_max_hp(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(19, "MatchInfo", "portal_max_hp")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`attack_damage` field](MatchInfo#structfield.attack_damage).
                #[inline]
                pub fn attack_damage(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(20, "MatchInfo", "attack_damage")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`recharge_amount` field](MatchInfo#structfield.recharge_amount).
                #[inline]
                pub fn recharge_amount(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(21, "MatchInfo", "recharge_amount")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`recharge_cost_supply` field](MatchInfo#structfield.recharge_cost_supply).
                #[inline]
                pub fn recharge_cost_supply(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(22, "MatchInfo", "recharge_cost_supply")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`hack_supply_yield` field](MatchInfo#structfield.hack_supply_yield).
                #[inline]
                pub fn hack_supply_yield(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(23, "MatchInfo", "hack_supply_yield")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`hack_key_yield_min` field](MatchInfo#structfield.hack_key_yield_min).
                #[inline]
                pub fn hack_key_yield_min(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(24, "MatchInfo", "hack_key_yield_min")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`hack_key_yield_max` field](MatchInfo#structfield.hack_key_yield_max).
                #[inline]
                pub fn hack_key_yield_max(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(25, "MatchInfo", "hack_key_yield_max")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`hack_cooldown_rounds` field](MatchInfo#structfield.hack_cooldown_rounds).
                #[inline]
                pub fn hack_cooldown_rounds(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(26, "MatchInfo", "hack_cooldown_rounds")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`hack_limit_per_epoch` field](MatchInfo#structfield.hack_limit_per_epoch).
                #[inline]
                pub fn hack_limit_per_epoch(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(27, "MatchInfo", "hack_limit_per_epoch")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`max_link_distance_cm` field](MatchInfo#structfield.max_link_distance_cm).
                #[inline]
                pub fn max_link_distance_cm(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(28, "MatchInfo", "max_link_distance_cm")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`max_commands_per_agent_round` field](MatchInfo#structfield.max_commands_per_agent_round).
                #[inline]
                pub fn max_commands_per_agent_round(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(29, "MatchInfo", "max_commands_per_agent_round")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`max_frame_payload_bytes` field](MatchInfo#structfield.max_frame_payload_bytes).
                #[inline]
                pub fn max_frame_payload_bytes(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(30, "MatchInfo", "max_frame_payload_bytes")?
                            .unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for MatchInfoRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("MatchInfoRef");
                    f.field("total_rounds", &self.total_rounds());
                    f.field("initial_spawn_window_ms", &self.initial_spawn_window_ms());
                    f.field("decision_window_ms", &self.decision_window_ms());
                    f.field("priority_bucket_ms", &self.priority_bucket_ms());
                    f.field("epoch_length", &self.epoch_length());
                    f.field("map_width_cm", &self.map_width_cm());
                    f.field("map_height_cm", &self.map_height_cm());
                    f.field("portal_count", &self.portal_count());
                    f.field("portal_min_distance_cm", &self.portal_min_distance_cm());
                    f.field(
                        "clustered_portal_ratio_permille",
                        &self.clustered_portal_ratio_permille(),
                    );
                    f.field("portal_cluster_count", &self.portal_cluster_count());
                    f.field(
                        "initial_agents_per_faction",
                        &self.initial_agents_per_faction(),
                    );
                    f.field(
                        "reinforcement_interval_rounds",
                        &self.reinforcement_interval_rounds(),
                    );
                    f.field(
                        "reinforcement_agents_per_faction",
                        &self.reinforcement_agents_per_faction(),
                    );
                    f.field("move_budget_cm", &self.move_budget_cm());
                    f.field(
                        "portal_operation_range_cm",
                        &self.portal_operation_range_cm(),
                    );
                    f.field("transfer_range_cm", &self.transfer_range_cm());
                    f.field("initial_supply", &self.initial_supply());
                    f.field("max_supply", &self.max_supply());
                    f.field("portal_max_hp", &self.portal_max_hp());
                    f.field("attack_damage", &self.attack_damage());
                    f.field("recharge_amount", &self.recharge_amount());
                    f.field("recharge_cost_supply", &self.recharge_cost_supply());
                    f.field("hack_supply_yield", &self.hack_supply_yield());
                    f.field("hack_key_yield_min", &self.hack_key_yield_min());
                    f.field("hack_key_yield_max", &self.hack_key_yield_max());
                    f.field("hack_cooldown_rounds", &self.hack_cooldown_rounds());
                    f.field("hack_limit_per_epoch", &self.hack_limit_per_epoch());
                    f.field("max_link_distance_cm", &self.max_link_distance_cm());
                    f.field(
                        "max_commands_per_agent_round",
                        &self.max_commands_per_agent_round(),
                    );
                    f.field("max_frame_payload_bytes", &self.max_frame_payload_bytes());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<MatchInfoRef<'a>> for MatchInfo {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: MatchInfoRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        total_rounds: ::core::convert::TryInto::try_into(value.total_rounds()?)?,
                        initial_spawn_window_ms: ::core::convert::TryInto::try_into(
                            value.initial_spawn_window_ms()?,
                        )?,
                        decision_window_ms: ::core::convert::TryInto::try_into(
                            value.decision_window_ms()?,
                        )?,
                        priority_bucket_ms: ::core::convert::TryInto::try_into(
                            value.priority_bucket_ms()?,
                        )?,
                        epoch_length: ::core::convert::TryInto::try_into(value.epoch_length()?)?,
                        map_width_cm: ::core::convert::TryInto::try_into(value.map_width_cm()?)?,
                        map_height_cm: ::core::convert::TryInto::try_into(value.map_height_cm()?)?,
                        portal_count: ::core::convert::TryInto::try_into(value.portal_count()?)?,
                        portal_min_distance_cm: ::core::convert::TryInto::try_into(
                            value.portal_min_distance_cm()?,
                        )?,
                        clustered_portal_ratio_permille: ::core::convert::TryInto::try_into(
                            value.clustered_portal_ratio_permille()?,
                        )?,
                        portal_cluster_count: ::core::convert::TryInto::try_into(
                            value.portal_cluster_count()?,
                        )?,
                        initial_agents_per_faction: ::core::convert::TryInto::try_into(
                            value.initial_agents_per_faction()?,
                        )?,
                        reinforcement_interval_rounds: ::core::convert::TryInto::try_into(
                            value.reinforcement_interval_rounds()?,
                        )?,
                        reinforcement_agents_per_faction: ::core::convert::TryInto::try_into(
                            value.reinforcement_agents_per_faction()?,
                        )?,
                        move_budget_cm: ::core::convert::TryInto::try_into(
                            value.move_budget_cm()?,
                        )?,
                        portal_operation_range_cm: ::core::convert::TryInto::try_into(
                            value.portal_operation_range_cm()?,
                        )?,
                        transfer_range_cm: ::core::convert::TryInto::try_into(
                            value.transfer_range_cm()?,
                        )?,
                        initial_supply: ::core::convert::TryInto::try_into(
                            value.initial_supply()?,
                        )?,
                        max_supply: ::core::convert::TryInto::try_into(value.max_supply()?)?,
                        portal_max_hp: ::core::convert::TryInto::try_into(value.portal_max_hp()?)?,
                        attack_damage: ::core::convert::TryInto::try_into(value.attack_damage()?)?,
                        recharge_amount: ::core::convert::TryInto::try_into(
                            value.recharge_amount()?,
                        )?,
                        recharge_cost_supply: ::core::convert::TryInto::try_into(
                            value.recharge_cost_supply()?,
                        )?,
                        hack_supply_yield: ::core::convert::TryInto::try_into(
                            value.hack_supply_yield()?,
                        )?,
                        hack_key_yield_min: ::core::convert::TryInto::try_into(
                            value.hack_key_yield_min()?,
                        )?,
                        hack_key_yield_max: ::core::convert::TryInto::try_into(
                            value.hack_key_yield_max()?,
                        )?,
                        hack_cooldown_rounds: ::core::convert::TryInto::try_into(
                            value.hack_cooldown_rounds()?,
                        )?,
                        hack_limit_per_epoch: ::core::convert::TryInto::try_into(
                            value.hack_limit_per_epoch()?,
                        )?,
                        max_link_distance_cm: ::core::convert::TryInto::try_into(
                            value.max_link_distance_cm()?,
                        )?,
                        max_commands_per_agent_round: ::core::convert::TryInto::try_into(
                            value.max_commands_per_agent_round()?,
                        )?,
                        max_frame_payload_bytes: ::core::convert::TryInto::try_into(
                            value.max_frame_payload_bytes()?,
                        )?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for MatchInfoRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for MatchInfoRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[MatchInfoRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<MatchInfo>> for MatchInfo {
                type Value = ::planus::Offset<MatchInfo>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<MatchInfo>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for MatchInfoRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[MatchInfoRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Server -> Client: the complete map has been atomically published to the
            ///  public SHM region (generation 0) and the pre-match Map View may be read;
            ///  the spawn window is running.
            ///
            /// Generated from these locations:
            /// * Table `InitialSpawnStart` in the file `crates/arena-protocol/schemas/protocol.fbs:104`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct InitialSpawnStart {
                /// The field `spawn_deadline_remaining_ms` in the table `InitialSpawnStart`
                pub spawn_deadline_remaining_ms: u32,
                /// The field `info` in the table `InitialSpawnStart`
                pub info: ::core::option::Option<::planus::alloc::boxed::Box<self::MatchInfo>>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for InitialSpawnStart {
                fn default() -> Self {
                    Self {
                        spawn_deadline_remaining_ms: 0,
                        info: ::core::default::Default::default(),
                    }
                }
            }

            impl InitialSpawnStart {
                /// Creates a [InitialSpawnStartBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> InitialSpawnStartBuilder<()> {
                    InitialSpawnStartBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_spawn_deadline_remaining_ms: impl ::planus::WriteAsDefault<u32, u32>,
                    field_info: impl ::planus::WriteAsOptional<::planus::Offset<self::MatchInfo>>,
                ) -> ::planus::Offset<Self> {
                    let prepared_spawn_deadline_remaining_ms =
                        field_spawn_deadline_remaining_ms.prepare(builder, &0);
                    let prepared_info = field_info.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_spawn_deadline_remaining_ms.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }
                    if prepared_info.is_some() {
                        table_writer.write_entry::<::planus::Offset<self::MatchInfo>>(1);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(
                                prepared_spawn_deadline_remaining_ms,
                            ) = prepared_spawn_deadline_remaining_ms
                            {
                                object_writer
                                    .write::<_, _, 4>(&prepared_spawn_deadline_remaining_ms);
                            }
                            if let ::core::option::Option::Some(prepared_info) = prepared_info {
                                object_writer.write::<_, _, 4>(&prepared_info);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<InitialSpawnStart>> for InitialSpawnStart {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnStart> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<InitialSpawnStart>> for InitialSpawnStart {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<InitialSpawnStart>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<InitialSpawnStart> for InitialSpawnStart {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnStart> {
                    InitialSpawnStart::create(builder, self.spawn_deadline_remaining_ms, &self.info)
                }
            }

            /// Builder for serializing an instance of the [InitialSpawnStart] type.
            ///
            /// Can be created using the [InitialSpawnStart::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct InitialSpawnStartBuilder<State>(State);

            impl InitialSpawnStartBuilder<()> {
                /// Setter for the [`spawn_deadline_remaining_ms` field](InitialSpawnStart#structfield.spawn_deadline_remaining_ms).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn spawn_deadline_remaining_ms<T0>(
                    self,
                    value: T0,
                ) -> InitialSpawnStartBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    InitialSpawnStartBuilder((value,))
                }

                /// Sets the [`spawn_deadline_remaining_ms` field](InitialSpawnStart#structfield.spawn_deadline_remaining_ms) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn spawn_deadline_remaining_ms_as_default(
                    self,
                ) -> InitialSpawnStartBuilder<(::planus::DefaultValue,)> {
                    self.spawn_deadline_remaining_ms(::planus::DefaultValue)
                }
            }

            impl<T0> InitialSpawnStartBuilder<(T0,)> {
                /// Setter for the [`info` field](InitialSpawnStart#structfield.info).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn info<T1>(self, value: T1) -> InitialSpawnStartBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsOptional<::planus::Offset<self::MatchInfo>>,
                {
                    let (v0,) = self.0;
                    InitialSpawnStartBuilder((v0, value))
                }

                /// Sets the [`info` field](InitialSpawnStart#structfield.info) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn info_as_null(self) -> InitialSpawnStartBuilder<(T0, ())> {
                    self.info(())
                }
            }

            impl<T0, T1> InitialSpawnStartBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [InitialSpawnStart].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnStart>
                where
                    Self: ::planus::WriteAsOffset<InitialSpawnStart>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsOptional<::planus::Offset<self::MatchInfo>>,
            > ::planus::WriteAs<::planus::Offset<InitialSpawnStart>>
                for InitialSpawnStartBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<InitialSpawnStart>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnStart> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsOptional<::planus::Offset<self::MatchInfo>>,
            > ::planus::WriteAsOptional<::planus::Offset<InitialSpawnStart>>
                for InitialSpawnStartBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<InitialSpawnStart>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<InitialSpawnStart>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsOptional<::planus::Offset<self::MatchInfo>>,
            > ::planus::WriteAsOffset<InitialSpawnStart> for InitialSpawnStartBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnStart> {
                    let (v0, v1) = &self.0;
                    InitialSpawnStart::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [InitialSpawnStart].
            #[derive(Copy, Clone)]
            pub struct InitialSpawnStartRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> InitialSpawnStartRef<'a> {
                /// Getter for the [`spawn_deadline_remaining_ms` field](InitialSpawnStart#structfield.spawn_deadline_remaining_ms).
                #[inline]
                pub fn spawn_deadline_remaining_ms(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(0, "InitialSpawnStart", "spawn_deadline_remaining_ms")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`info` field](InitialSpawnStart#structfield.info).
                #[inline]
                pub fn info(
                    &self,
                ) -> ::planus::Result<::core::option::Option<self::MatchInfoRef<'a>>>
                {
                    self.0.access(1, "InitialSpawnStart", "info")
                }
            }

            impl<'a> ::core::fmt::Debug for InitialSpawnStartRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("InitialSpawnStartRef");
                    f.field(
                        "spawn_deadline_remaining_ms",
                        &self.spawn_deadline_remaining_ms(),
                    );
                    if let ::core::option::Option::Some(field_info) = self.info().transpose() {
                        f.field("info", &field_info);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<InitialSpawnStartRef<'a>> for InitialSpawnStart {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: InitialSpawnStartRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        spawn_deadline_remaining_ms: ::core::convert::TryInto::try_into(
                            value.spawn_deadline_remaining_ms()?,
                        )?,
                        info: if let ::core::option::Option::Some(info) = value.info()? {
                            ::core::option::Option::Some(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryInto::try_into(info)?,
                            ))
                        } else {
                            ::core::option::Option::None
                        },
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for InitialSpawnStartRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for InitialSpawnStartRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[InitialSpawnStartRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<InitialSpawnStart>> for InitialSpawnStart {
                type Value = ::planus::Offset<InitialSpawnStart>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<InitialSpawnStart>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for InitialSpawnStartRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[InitialSpawnStartRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Client -> Server, once per faction inside the spawn window: the complete
            ///  agent_id -> coordinate set, validated as a whole (R-SPAWN-01).
            ///
            /// Generated from these locations:
            /// * Table `SubmitInitialSpawns` in the file `crates/arena-protocol/schemas/protocol.fbs:111`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct SubmitInitialSpawns {
                /// The field `positions` in the table `SubmitInitialSpawns`
                pub positions: ::core::option::Option<::planus::alloc::vec::Vec<self::SpawnEntry>>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for SubmitInitialSpawns {
                fn default() -> Self {
                    Self {
                        positions: ::core::default::Default::default(),
                    }
                }
            }

            impl SubmitInitialSpawns {
                /// Creates a [SubmitInitialSpawnsBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> SubmitInitialSpawnsBuilder<()> {
                    SubmitInitialSpawnsBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_positions: impl ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::SpawnEntry>]>,
                    >,
                ) -> ::planus::Offset<Self> {
                    let prepared_positions = field_positions.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_positions.is_some() {
                        table_writer
                            .write_entry::<::planus::Offset<[::planus::Offset<self::SpawnEntry>]>>(
                                0,
                            );
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_positions) =
                                prepared_positions
                            {
                                object_writer.write::<_, _, 4>(&prepared_positions);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<SubmitInitialSpawns>> for SubmitInitialSpawns {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitInitialSpawns> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<SubmitInitialSpawns>> for SubmitInitialSpawns {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SubmitInitialSpawns>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<SubmitInitialSpawns> for SubmitInitialSpawns {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitInitialSpawns> {
                    SubmitInitialSpawns::create(builder, &self.positions)
                }
            }

            /// Builder for serializing an instance of the [SubmitInitialSpawns] type.
            ///
            /// Can be created using the [SubmitInitialSpawns::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct SubmitInitialSpawnsBuilder<State>(State);

            impl SubmitInitialSpawnsBuilder<()> {
                /// Setter for the [`positions` field](SubmitInitialSpawns#structfield.positions).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn positions<T0>(self, value: T0) -> SubmitInitialSpawnsBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::SpawnEntry>]>,
                        >,
                {
                    SubmitInitialSpawnsBuilder((value,))
                }

                /// Sets the [`positions` field](SubmitInitialSpawns#structfield.positions) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn positions_as_null(self) -> SubmitInitialSpawnsBuilder<((),)> {
                    self.positions(())
                }
            }

            impl<T0> SubmitInitialSpawnsBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [SubmitInitialSpawns].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitInitialSpawns>
                where
                    Self: ::planus::WriteAsOffset<SubmitInitialSpawns>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::SpawnEntry>]>>,
            > ::planus::WriteAs<::planus::Offset<SubmitInitialSpawns>>
                for SubmitInitialSpawnsBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<SubmitInitialSpawns>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitInitialSpawns> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::SpawnEntry>]>>,
            > ::planus::WriteAsOptional<::planus::Offset<SubmitInitialSpawns>>
                for SubmitInitialSpawnsBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<SubmitInitialSpawns>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SubmitInitialSpawns>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::SpawnEntry>]>>,
            > ::planus::WriteAsOffset<SubmitInitialSpawns> for SubmitInitialSpawnsBuilder<(T0,)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitInitialSpawns> {
                    let (v0,) = &self.0;
                    SubmitInitialSpawns::create(builder, v0)
                }
            }

            /// Reference to a deserialized [SubmitInitialSpawns].
            #[derive(Copy, Clone)]
            pub struct SubmitInitialSpawnsRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> SubmitInitialSpawnsRef<'a> {
                /// Getter for the [`positions` field](SubmitInitialSpawns#structfield.positions).
                #[inline]
                pub fn positions(
                    &self,
                ) -> ::planus::Result<
                    ::core::option::Option<
                        ::planus::Vector<'a, ::planus::Result<self::SpawnEntryRef<'a>>>,
                    >,
                > {
                    self.0.access(0, "SubmitInitialSpawns", "positions")
                }
            }

            impl<'a> ::core::fmt::Debug for SubmitInitialSpawnsRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("SubmitInitialSpawnsRef");
                    if let ::core::option::Option::Some(field_positions) =
                        self.positions().transpose()
                    {
                        f.field("positions", &field_positions);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<SubmitInitialSpawnsRef<'a>> for SubmitInitialSpawns {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: SubmitInitialSpawnsRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        positions: if let ::core::option::Option::Some(positions) =
                            value.positions()?
                        {
                            ::core::option::Option::Some(positions.to_vec_result()?)
                        } else {
                            ::core::option::Option::None
                        },
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for SubmitInitialSpawnsRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for SubmitInitialSpawnsRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[SubmitInitialSpawnsRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<SubmitInitialSpawns>> for SubmitInitialSpawns {
                type Value = ::planus::Offset<SubmitInitialSpawns>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<SubmitInitialSpawns>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for SubmitInitialSpawnsRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[SubmitInitialSpawnsRef]",
                            "read_as_root",
                            0,
                        )
                    })
                }
            }

            /// The table `SpawnEntry` in the namespace `ingress.protocol`
            ///
            /// Generated from these locations:
            /// * Table `SpawnEntry` in the file `crates/arena-protocol/schemas/protocol.fbs:115`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct SpawnEntry {
                /// The field `agent_id` in the table `SpawnEntry`
                pub agent_id: u32,
                /// The field `x` in the table `SpawnEntry`
                pub x: i32,
                /// The field `y` in the table `SpawnEntry`
                pub y: i32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for SpawnEntry {
                fn default() -> Self {
                    Self {
                        agent_id: 0,
                        x: 0,
                        y: 0,
                    }
                }
            }

            impl SpawnEntry {
                /// Creates a [SpawnEntryBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> SpawnEntryBuilder<()> {
                    SpawnEntryBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_agent_id: impl ::planus::WriteAsDefault<u32, u32>,
                    field_x: impl ::planus::WriteAsDefault<i32, i32>,
                    field_y: impl ::planus::WriteAsDefault<i32, i32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_agent_id = field_agent_id.prepare(builder, &0);
                    let prepared_x = field_x.prepare(builder, &0);
                    let prepared_y = field_y.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<10> =
                        ::core::default::Default::default();
                    if prepared_agent_id.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }
                    if prepared_x.is_some() {
                        table_writer.write_entry::<i32>(1);
                    }
                    if prepared_y.is_some() {
                        table_writer.write_entry::<i32>(2);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_agent_id) =
                                prepared_agent_id
                            {
                                object_writer.write::<_, _, 4>(&prepared_agent_id);
                            }
                            if let ::core::option::Option::Some(prepared_x) = prepared_x {
                                object_writer.write::<_, _, 4>(&prepared_x);
                            }
                            if let ::core::option::Option::Some(prepared_y) = prepared_y {
                                object_writer.write::<_, _, 4>(&prepared_y);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<SpawnEntry>> for SpawnEntry {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpawnEntry> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<SpawnEntry>> for SpawnEntry {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SpawnEntry>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<SpawnEntry> for SpawnEntry {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpawnEntry> {
                    SpawnEntry::create(builder, self.agent_id, self.x, self.y)
                }
            }

            /// Builder for serializing an instance of the [SpawnEntry] type.
            ///
            /// Can be created using the [SpawnEntry::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct SpawnEntryBuilder<State>(State);

            impl SpawnEntryBuilder<()> {
                /// Setter for the [`agent_id` field](SpawnEntry#structfield.agent_id).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn agent_id<T0>(self, value: T0) -> SpawnEntryBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    SpawnEntryBuilder((value,))
                }

                /// Sets the [`agent_id` field](SpawnEntry#structfield.agent_id) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn agent_id_as_default(self) -> SpawnEntryBuilder<(::planus::DefaultValue,)> {
                    self.agent_id(::planus::DefaultValue)
                }
            }

            impl<T0> SpawnEntryBuilder<(T0,)> {
                /// Setter for the [`x` field](SpawnEntry#structfield.x).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn x<T1>(self, value: T1) -> SpawnEntryBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<i32, i32>,
                {
                    let (v0,) = self.0;
                    SpawnEntryBuilder((v0, value))
                }

                /// Sets the [`x` field](SpawnEntry#structfield.x) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn x_as_default(self) -> SpawnEntryBuilder<(T0, ::planus::DefaultValue)> {
                    self.x(::planus::DefaultValue)
                }
            }

            impl<T0, T1> SpawnEntryBuilder<(T0, T1)> {
                /// Setter for the [`y` field](SpawnEntry#structfield.y).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn y<T2>(self, value: T2) -> SpawnEntryBuilder<(T0, T1, T2)>
                where
                    T2: ::planus::WriteAsDefault<i32, i32>,
                {
                    let (v0, v1) = self.0;
                    SpawnEntryBuilder((v0, v1, value))
                }

                /// Sets the [`y` field](SpawnEntry#structfield.y) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn y_as_default(self) -> SpawnEntryBuilder<(T0, T1, ::planus::DefaultValue)> {
                    self.y(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2> SpawnEntryBuilder<(T0, T1, T2)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [SpawnEntry].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpawnEntry>
                where
                    Self: ::planus::WriteAsOffset<SpawnEntry>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<i32, i32>,
                T2: ::planus::WriteAsDefault<i32, i32>,
            > ::planus::WriteAs<::planus::Offset<SpawnEntry>> for SpawnEntryBuilder<(T0, T1, T2)>
            {
                type Prepared = ::planus::Offset<SpawnEntry>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpawnEntry> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<i32, i32>,
                T2: ::planus::WriteAsDefault<i32, i32>,
            > ::planus::WriteAsOptional<::planus::Offset<SpawnEntry>>
                for SpawnEntryBuilder<(T0, T1, T2)>
            {
                type Prepared = ::planus::Offset<SpawnEntry>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SpawnEntry>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<i32, i32>,
                T2: ::planus::WriteAsDefault<i32, i32>,
            > ::planus::WriteAsOffset<SpawnEntry> for SpawnEntryBuilder<(T0, T1, T2)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpawnEntry> {
                    let (v0, v1, v2) = &self.0;
                    SpawnEntry::create(builder, v0, v1, v2)
                }
            }

            /// Reference to a deserialized [SpawnEntry].
            #[derive(Copy, Clone)]
            pub struct SpawnEntryRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> SpawnEntryRef<'a> {
                /// Getter for the [`agent_id` field](SpawnEntry#structfield.agent_id).
                #[inline]
                pub fn agent_id(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "SpawnEntry", "agent_id")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`x` field](SpawnEntry#structfield.x).
                #[inline]
                pub fn x(&self) -> ::planus::Result<i32> {
                    ::core::result::Result::Ok(self.0.access(1, "SpawnEntry", "x")?.unwrap_or(0))
                }

                /// Getter for the [`y` field](SpawnEntry#structfield.y).
                #[inline]
                pub fn y(&self) -> ::planus::Result<i32> {
                    ::core::result::Result::Ok(self.0.access(2, "SpawnEntry", "y")?.unwrap_or(0))
                }
            }

            impl<'a> ::core::fmt::Debug for SpawnEntryRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("SpawnEntryRef");
                    f.field("agent_id", &self.agent_id());
                    f.field("x", &self.x());
                    f.field("y", &self.y());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<SpawnEntryRef<'a>> for SpawnEntry {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: SpawnEntryRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        agent_id: ::core::convert::TryInto::try_into(value.agent_id()?)?,
                        x: ::core::convert::TryInto::try_into(value.x()?)?,
                        y: ::core::convert::TryInto::try_into(value.y()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for SpawnEntryRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for SpawnEntryRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[SpawnEntryRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<SpawnEntry>> for SpawnEntry {
                type Value = ::planus::Offset<SpawnEntry>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<SpawnEntry>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for SpawnEntryRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[SpawnEntryRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Server -> Client. status: 0 = accepted, 1 = invalid set (SPAWN_FORFEIT),
            ///  2 = missed deadline (SPAWN_FORFEIT). Private reason; no replacement.
            ///
            /// Generated from these locations:
            /// * Table `InitialSpawnResult` in the file `crates/arena-protocol/schemas/protocol.fbs:123`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct InitialSpawnResult {
                /// The field `status` in the table `InitialSpawnResult`
                pub status: u8,
                /// The field `detail` in the table `InitialSpawnResult`
                pub detail: ::core::option::Option<::planus::alloc::string::String>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for InitialSpawnResult {
                fn default() -> Self {
                    Self {
                        status: 0,
                        detail: ::core::default::Default::default(),
                    }
                }
            }

            impl InitialSpawnResult {
                /// Creates a [InitialSpawnResultBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> InitialSpawnResultBuilder<()> {
                    InitialSpawnResultBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_status: impl ::planus::WriteAsDefault<u8, u8>,
                    field_detail: impl ::planus::WriteAsOptional<
                        ::planus::Offset<::core::primitive::str>,
                    >,
                ) -> ::planus::Offset<Self> {
                    let prepared_status = field_status.prepare(builder, &0);
                    let prepared_detail = field_detail.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_detail.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(1);
                    }
                    if prepared_status.is_some() {
                        table_writer.write_entry::<u8>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_detail) = prepared_detail {
                                object_writer.write::<_, _, 4>(&prepared_detail);
                            }
                            if let ::core::option::Option::Some(prepared_status) = prepared_status {
                                object_writer.write::<_, _, 1>(&prepared_status);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<InitialSpawnResult>> for InitialSpawnResult {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnResult> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<InitialSpawnResult>> for InitialSpawnResult {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<InitialSpawnResult>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<InitialSpawnResult> for InitialSpawnResult {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnResult> {
                    InitialSpawnResult::create(builder, self.status, &self.detail)
                }
            }

            /// Builder for serializing an instance of the [InitialSpawnResult] type.
            ///
            /// Can be created using the [InitialSpawnResult::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct InitialSpawnResultBuilder<State>(State);

            impl InitialSpawnResultBuilder<()> {
                /// Setter for the [`status` field](InitialSpawnResult#structfield.status).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn status<T0>(self, value: T0) -> InitialSpawnResultBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u8, u8>,
                {
                    InitialSpawnResultBuilder((value,))
                }

                /// Sets the [`status` field](InitialSpawnResult#structfield.status) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn status_as_default(
                    self,
                ) -> InitialSpawnResultBuilder<(::planus::DefaultValue,)> {
                    self.status(::planus::DefaultValue)
                }
            }

            impl<T0> InitialSpawnResultBuilder<(T0,)> {
                /// Setter for the [`detail` field](InitialSpawnResult#structfield.detail).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn detail<T1>(self, value: T1) -> InitialSpawnResultBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    let (v0,) = self.0;
                    InitialSpawnResultBuilder((v0, value))
                }

                /// Sets the [`detail` field](InitialSpawnResult#structfield.detail) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn detail_as_null(self) -> InitialSpawnResultBuilder<(T0, ())> {
                    self.detail(())
                }
            }

            impl<T0, T1> InitialSpawnResultBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [InitialSpawnResult].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnResult>
                where
                    Self: ::planus::WriteAsOffset<InitialSpawnResult>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
            > ::planus::WriteAs<::planus::Offset<InitialSpawnResult>>
                for InitialSpawnResultBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<InitialSpawnResult>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnResult> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
            > ::planus::WriteAsOptional<::planus::Offset<InitialSpawnResult>>
                for InitialSpawnResultBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<InitialSpawnResult>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<InitialSpawnResult>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
            > ::planus::WriteAsOffset<InitialSpawnResult> for InitialSpawnResultBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<InitialSpawnResult> {
                    let (v0, v1) = &self.0;
                    InitialSpawnResult::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [InitialSpawnResult].
            #[derive(Copy, Clone)]
            pub struct InitialSpawnResultRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> InitialSpawnResultRef<'a> {
                /// Getter for the [`status` field](InitialSpawnResult#structfield.status).
                #[inline]
                pub fn status(&self) -> ::planus::Result<u8> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(0, "InitialSpawnResult", "status")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`detail` field](InitialSpawnResult#structfield.detail).
                #[inline]
                pub fn detail(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(1, "InitialSpawnResult", "detail")
                }
            }

            impl<'a> ::core::fmt::Debug for InitialSpawnResultRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("InitialSpawnResultRef");
                    f.field("status", &self.status());
                    if let ::core::option::Option::Some(field_detail) = self.detail().transpose() {
                        f.field("detail", &field_detail);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<InitialSpawnResultRef<'a>> for InitialSpawnResult {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: InitialSpawnResultRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        status: ::core::convert::TryInto::try_into(value.status()?)?,
                        detail: value.detail()?.map(::core::convert::Into::into),
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for InitialSpawnResultRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for InitialSpawnResultRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[InitialSpawnResultRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<InitialSpawnResult>> for InitialSpawnResult {
                type Value = ::planus::Offset<InitialSpawnResult>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<InitialSpawnResult>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for InitialSpawnResultRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[InitialSpawnResultRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Server -> Client: a new snapshot generation is published and SHM reads
            ///  are licensed again. No authoritative round payload (P3-RPC).
            ///
            /// Generated from these locations:
            /// * Table `RoundStartSignal` in the file `crates/arena-protocol/schemas/protocol.fbs:130`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct RoundStartSignal {}

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for RoundStartSignal {
                fn default() -> Self {
                    Self {}
                }
            }

            impl RoundStartSignal {
                /// Creates a [RoundStartSignalBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> RoundStartSignalBuilder<()> {
                    RoundStartSignalBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(builder: &mut ::planus::Builder) -> ::planus::Offset<Self> {
                    let table_writer: ::planus::table_writer::TableWriter<4> =
                        ::core::default::Default::default();
                    unsafe {
                        table_writer.finish(builder, |_table_writer| {});
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<RoundStartSignal>> for RoundStartSignal {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundStartSignal> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<RoundStartSignal>> for RoundStartSignal {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<RoundStartSignal>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<RoundStartSignal> for RoundStartSignal {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundStartSignal> {
                    RoundStartSignal::create(builder)
                }
            }

            /// Builder for serializing an instance of the [RoundStartSignal] type.
            ///
            /// Can be created using the [RoundStartSignal::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct RoundStartSignalBuilder<State>(State);

            impl RoundStartSignalBuilder<()> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [RoundStartSignal].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundStartSignal>
                where
                    Self: ::planus::WriteAsOffset<RoundStartSignal>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl ::planus::WriteAs<::planus::Offset<RoundStartSignal>> for RoundStartSignalBuilder<()> {
                type Prepared = ::planus::Offset<RoundStartSignal>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundStartSignal> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<RoundStartSignal>> for RoundStartSignalBuilder<()> {
                type Prepared = ::planus::Offset<RoundStartSignal>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<RoundStartSignal>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<RoundStartSignal> for RoundStartSignalBuilder<()> {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundStartSignal> {
                    RoundStartSignal::create(builder)
                }
            }

            /// Reference to a deserialized [RoundStartSignal].
            #[derive(Copy, Clone)]
            pub struct RoundStartSignalRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> RoundStartSignalRef<'a> {}

            impl<'a> ::core::fmt::Debug for RoundStartSignalRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("RoundStartSignalRef");

                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<RoundStartSignalRef<'a>> for RoundStartSignal {
                type Error = ::planus::Error;

                fn try_from(_value: RoundStartSignalRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {})
                }
            }

            impl<'a> ::planus::TableRead<'a> for RoundStartSignalRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for RoundStartSignalRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[RoundStartSignalRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<RoundStartSignal>> for RoundStartSignal {
                type Value = ::planus::Offset<RoundStartSignal>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<RoundStartSignal>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for RoundStartSignalRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[RoundStartSignalRef]", "read_as_root", 0)
                    })
                }
            }

            ///  One agent's ordered command stream for one round. Timing is server
            ///  side: the frame's last byte arrival stamps `received_at`; clients send
            ///  no stream ids or nonces.
            ///
            /// Generated from these locations:
            /// * Union `CommandBody` in the file `crates/arena-protocol/schemas/protocol.fbs:135`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub enum CommandBody {
                /// The variant `Move` in the union `CommandBody`
                Move(::planus::alloc::boxed::Box<self::CmdMove>),

                /// The variant `Attack` in the union `CommandBody`
                Attack(::planus::alloc::boxed::Box<self::CmdEmpty>),

                /// The variant `Capture` in the union `CommandBody`
                Capture(::planus::alloc::boxed::Box<self::CmdPortal>),

                /// The variant `Recharge` in the union `CommandBody`
                Recharge(::planus::alloc::boxed::Box<self::CmdPortal>),

                /// The variant `Hack` in the union `CommandBody`
                Hack(::planus::alloc::boxed::Box<self::CmdPortal>),

                /// The variant `Link` in the union `CommandBody`
                Link(::planus::alloc::boxed::Box<self::CmdLink>),

                /// The variant `Transfer` in the union `CommandBody`
                Transfer(::planus::alloc::boxed::Box<self::CmdTransfer>),
            }

            impl CommandBody {
                /// Creates a [CommandBodyBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> CommandBodyBuilder<::planus::Uninitialized> {
                    CommandBodyBuilder(::planus::Uninitialized)
                }

                #[inline]
                pub fn create_move(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::CmdMove>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(1, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_attack(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::CmdEmpty>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(2, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_capture(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::CmdPortal>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(3, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_recharge(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::CmdPortal>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(4, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_hack(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::CmdPortal>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(5, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_link(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::CmdLink>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(6, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_transfer(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::CmdTransfer>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(7, value.prepare(builder).downcast())
                }
            }

            impl ::planus::WriteAsUnion<CommandBody> for CommandBody {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::UnionOffset<Self> {
                    match self {
                        Self::Move(value) => Self::create_move(builder, value),
                        Self::Attack(value) => Self::create_attack(builder, value),
                        Self::Capture(value) => Self::create_capture(builder, value),
                        Self::Recharge(value) => Self::create_recharge(builder, value),
                        Self::Hack(value) => Self::create_hack(builder, value),
                        Self::Link(value) => Self::create_link(builder, value),
                        Self::Transfer(value) => Self::create_transfer(builder, value),
                    }
                }
            }

            impl ::planus::WriteAsOptionalUnion<CommandBody> for CommandBody {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Self>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }

            /// Builder for serializing an instance of the [CommandBody] type.
            ///
            /// Can be created using the [CommandBody::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct CommandBodyBuilder<T>(T);

            impl CommandBodyBuilder<::planus::Uninitialized> {
                /// Creates an instance of the [`Move` variant](CommandBody#variant.Move).
                #[inline]
                pub fn move_<T>(self, value: T) -> CommandBodyBuilder<::planus::Initialized<1, T>>
                where
                    T: ::planus::WriteAsOffset<self::CmdMove>,
                {
                    CommandBodyBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`Attack` variant](CommandBody#variant.Attack).
                #[inline]
                pub fn attack<T>(self, value: T) -> CommandBodyBuilder<::planus::Initialized<2, T>>
                where
                    T: ::planus::WriteAsOffset<self::CmdEmpty>,
                {
                    CommandBodyBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`Capture` variant](CommandBody#variant.Capture).
                #[inline]
                pub fn capture<T>(self, value: T) -> CommandBodyBuilder<::planus::Initialized<3, T>>
                where
                    T: ::planus::WriteAsOffset<self::CmdPortal>,
                {
                    CommandBodyBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`Recharge` variant](CommandBody#variant.Recharge).
                #[inline]
                pub fn recharge<T>(
                    self,
                    value: T,
                ) -> CommandBodyBuilder<::planus::Initialized<4, T>>
                where
                    T: ::planus::WriteAsOffset<self::CmdPortal>,
                {
                    CommandBodyBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`Hack` variant](CommandBody#variant.Hack).
                #[inline]
                pub fn hack<T>(self, value: T) -> CommandBodyBuilder<::planus::Initialized<5, T>>
                where
                    T: ::planus::WriteAsOffset<self::CmdPortal>,
                {
                    CommandBodyBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`Link` variant](CommandBody#variant.Link).
                #[inline]
                pub fn link<T>(self, value: T) -> CommandBodyBuilder<::planus::Initialized<6, T>>
                where
                    T: ::planus::WriteAsOffset<self::CmdLink>,
                {
                    CommandBodyBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`Transfer` variant](CommandBody#variant.Transfer).
                #[inline]
                pub fn transfer<T>(
                    self,
                    value: T,
                ) -> CommandBodyBuilder<::planus::Initialized<7, T>>
                where
                    T: ::planus::WriteAsOffset<self::CmdTransfer>,
                {
                    CommandBodyBuilder(::planus::Initialized(value))
                }
            }

            impl<const N: u8, T> CommandBodyBuilder<::planus::Initialized<N, T>> {
                /// Finish writing the builder to get an [UnionOffset](::planus::UnionOffset) to a serialized [CommandBody].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<CommandBody>
                where
                    Self: ::planus::WriteAsUnion<CommandBody>,
                {
                    ::planus::WriteAsUnion::prepare(&self, builder)
                }
            }

            impl<T> ::planus::WriteAsUnion<CommandBody> for CommandBodyBuilder<::planus::Initialized<1, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdMove>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<CommandBody> {
                    ::planus::UnionOffset::new(1, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<CommandBody>
                for CommandBodyBuilder<::planus::Initialized<1, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdMove>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<CommandBody>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<CommandBody> for CommandBodyBuilder<::planus::Initialized<2, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdEmpty>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<CommandBody> {
                    ::planus::UnionOffset::new(2, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<CommandBody>
                for CommandBodyBuilder<::planus::Initialized<2, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdEmpty>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<CommandBody>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<CommandBody> for CommandBodyBuilder<::planus::Initialized<3, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdPortal>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<CommandBody> {
                    ::planus::UnionOffset::new(3, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<CommandBody>
                for CommandBodyBuilder<::planus::Initialized<3, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdPortal>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<CommandBody>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<CommandBody> for CommandBodyBuilder<::planus::Initialized<4, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdPortal>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<CommandBody> {
                    ::planus::UnionOffset::new(4, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<CommandBody>
                for CommandBodyBuilder<::planus::Initialized<4, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdPortal>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<CommandBody>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<CommandBody> for CommandBodyBuilder<::planus::Initialized<5, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdPortal>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<CommandBody> {
                    ::planus::UnionOffset::new(5, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<CommandBody>
                for CommandBodyBuilder<::planus::Initialized<5, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdPortal>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<CommandBody>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<CommandBody> for CommandBodyBuilder<::planus::Initialized<6, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdLink>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<CommandBody> {
                    ::planus::UnionOffset::new(6, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<CommandBody>
                for CommandBodyBuilder<::planus::Initialized<6, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdLink>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<CommandBody>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<CommandBody> for CommandBodyBuilder<::planus::Initialized<7, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdTransfer>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<CommandBody> {
                    ::planus::UnionOffset::new(7, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<CommandBody>
                for CommandBodyBuilder<::planus::Initialized<7, T>>
            where
                T: ::planus::WriteAsOffset<self::CmdTransfer>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<CommandBody>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }

            /// Reference to a deserialized [CommandBody].
            #[derive(Copy, Clone, Debug)]
            pub enum CommandBodyRef<'a> {
                Move(self::CmdMoveRef<'a>),
                Attack(self::CmdEmptyRef<'a>),
                Capture(self::CmdPortalRef<'a>),
                Recharge(self::CmdPortalRef<'a>),
                Hack(self::CmdPortalRef<'a>),
                Link(self::CmdLinkRef<'a>),
                Transfer(self::CmdTransferRef<'a>),
            }

            impl<'a> ::core::convert::TryFrom<CommandBodyRef<'a>> for CommandBody {
                type Error = ::planus::Error;

                fn try_from(value: CommandBodyRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(match value {
                        CommandBodyRef::Move(value) => {
                            Self::Move(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        CommandBodyRef::Attack(value) => {
                            Self::Attack(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        CommandBodyRef::Capture(value) => {
                            Self::Capture(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        CommandBodyRef::Recharge(value) => {
                            Self::Recharge(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        CommandBodyRef::Hack(value) => {
                            Self::Hack(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        CommandBodyRef::Link(value) => {
                            Self::Link(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }

                        CommandBodyRef::Transfer(value) => {
                            Self::Transfer(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryFrom::try_from(value)?,
                            ))
                        }
                    })
                }
            }

            impl<'a> ::planus::TableReadUnion<'a> for CommandBodyRef<'a> {
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    tag: u8,
                    field_offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    match tag {
                        1 => ::core::result::Result::Ok(Self::Move(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        2 => ::core::result::Result::Ok(Self::Attack(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        3 => ::core::result::Result::Ok(Self::Capture(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        4 => ::core::result::Result::Ok(Self::Recharge(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        5 => ::core::result::Result::Ok(Self::Hack(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        6 => ::core::result::Result::Ok(Self::Link(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        7 => ::core::result::Result::Ok(Self::Transfer(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        _ => ::core::result::Result::Err(
                            ::planus::errors::ErrorKind::UnknownUnionTag { tag },
                        ),
                    }
                }
            }

            impl<'a> ::planus::VectorReadUnion<'a> for CommandBodyRef<'a> {
                const VECTOR_NAME: &'static str = "[CommandBodyRef]";
            }

            /// The table `CmdMove` in the namespace `ingress.protocol`
            ///
            /// Generated from these locations:
            /// * Table `CmdMove` in the file `crates/arena-protocol/schemas/protocol.fbs:145`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct CmdMove {
                /// The field `x` in the table `CmdMove`
                pub x: i32,
                /// The field `y` in the table `CmdMove`
                pub y: i32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for CmdMove {
                fn default() -> Self {
                    Self { x: 0, y: 0 }
                }
            }

            impl CmdMove {
                /// Creates a [CmdMoveBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> CmdMoveBuilder<()> {
                    CmdMoveBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_x: impl ::planus::WriteAsDefault<i32, i32>,
                    field_y: impl ::planus::WriteAsDefault<i32, i32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_x = field_x.prepare(builder, &0);
                    let prepared_y = field_y.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_x.is_some() {
                        table_writer.write_entry::<i32>(0);
                    }
                    if prepared_y.is_some() {
                        table_writer.write_entry::<i32>(1);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_x) = prepared_x {
                                object_writer.write::<_, _, 4>(&prepared_x);
                            }
                            if let ::core::option::Option::Some(prepared_y) = prepared_y {
                                object_writer.write::<_, _, 4>(&prepared_y);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<CmdMove>> for CmdMove {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdMove> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<CmdMove>> for CmdMove {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdMove>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<CmdMove> for CmdMove {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdMove> {
                    CmdMove::create(builder, self.x, self.y)
                }
            }

            /// Builder for serializing an instance of the [CmdMove] type.
            ///
            /// Can be created using the [CmdMove::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct CmdMoveBuilder<State>(State);

            impl CmdMoveBuilder<()> {
                /// Setter for the [`x` field](CmdMove#structfield.x).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn x<T0>(self, value: T0) -> CmdMoveBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<i32, i32>,
                {
                    CmdMoveBuilder((value,))
                }

                /// Sets the [`x` field](CmdMove#structfield.x) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn x_as_default(self) -> CmdMoveBuilder<(::planus::DefaultValue,)> {
                    self.x(::planus::DefaultValue)
                }
            }

            impl<T0> CmdMoveBuilder<(T0,)> {
                /// Setter for the [`y` field](CmdMove#structfield.y).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn y<T1>(self, value: T1) -> CmdMoveBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<i32, i32>,
                {
                    let (v0,) = self.0;
                    CmdMoveBuilder((v0, value))
                }

                /// Sets the [`y` field](CmdMove#structfield.y) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn y_as_default(self) -> CmdMoveBuilder<(T0, ::planus::DefaultValue)> {
                    self.y(::planus::DefaultValue)
                }
            }

            impl<T0, T1> CmdMoveBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [CmdMove].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdMove>
                where
                    Self: ::planus::WriteAsOffset<CmdMove>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<i32, i32>, T1: ::planus::WriteAsDefault<i32, i32>>
                ::planus::WriteAs<::planus::Offset<CmdMove>> for CmdMoveBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<CmdMove>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdMove> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<i32, i32>, T1: ::planus::WriteAsDefault<i32, i32>>
                ::planus::WriteAsOptional<::planus::Offset<CmdMove>> for CmdMoveBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<CmdMove>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdMove>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<i32, i32>, T1: ::planus::WriteAsDefault<i32, i32>>
                ::planus::WriteAsOffset<CmdMove> for CmdMoveBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdMove> {
                    let (v0, v1) = &self.0;
                    CmdMove::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [CmdMove].
            #[derive(Copy, Clone)]
            pub struct CmdMoveRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> CmdMoveRef<'a> {
                /// Getter for the [`x` field](CmdMove#structfield.x).
                #[inline]
                pub fn x(&self) -> ::planus::Result<i32> {
                    ::core::result::Result::Ok(self.0.access(0, "CmdMove", "x")?.unwrap_or(0))
                }

                /// Getter for the [`y` field](CmdMove#structfield.y).
                #[inline]
                pub fn y(&self) -> ::planus::Result<i32> {
                    ::core::result::Result::Ok(self.0.access(1, "CmdMove", "y")?.unwrap_or(0))
                }
            }

            impl<'a> ::core::fmt::Debug for CmdMoveRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("CmdMoveRef");
                    f.field("x", &self.x());
                    f.field("y", &self.y());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<CmdMoveRef<'a>> for CmdMove {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: CmdMoveRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        x: ::core::convert::TryInto::try_into(value.x()?)?,
                        y: ::core::convert::TryInto::try_into(value.y()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for CmdMoveRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for CmdMoveRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[CmdMoveRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<CmdMove>> for CmdMove {
                type Value = ::planus::Offset<CmdMove>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<CmdMove>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for CmdMoveRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[CmdMoveRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `CmdPortal` in the namespace `ingress.protocol`
            ///
            /// Generated from these locations:
            /// * Table `CmdPortal` in the file `crates/arena-protocol/schemas/protocol.fbs:146`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct CmdPortal {
                /// The field `portal` in the table `CmdPortal`
                pub portal: u32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for CmdPortal {
                fn default() -> Self {
                    Self { portal: 0 }
                }
            }

            impl CmdPortal {
                /// Creates a [CmdPortalBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> CmdPortalBuilder<()> {
                    CmdPortalBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_portal: impl ::planus::WriteAsDefault<u32, u32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_portal = field_portal.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_portal.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_portal) = prepared_portal {
                                object_writer.write::<_, _, 4>(&prepared_portal);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<CmdPortal>> for CmdPortal {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdPortal> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<CmdPortal>> for CmdPortal {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdPortal>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<CmdPortal> for CmdPortal {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdPortal> {
                    CmdPortal::create(builder, self.portal)
                }
            }

            /// Builder for serializing an instance of the [CmdPortal] type.
            ///
            /// Can be created using the [CmdPortal::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct CmdPortalBuilder<State>(State);

            impl CmdPortalBuilder<()> {
                /// Setter for the [`portal` field](CmdPortal#structfield.portal).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal<T0>(self, value: T0) -> CmdPortalBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    CmdPortalBuilder((value,))
                }

                /// Sets the [`portal` field](CmdPortal#structfield.portal) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_as_default(self) -> CmdPortalBuilder<(::planus::DefaultValue,)> {
                    self.portal(::planus::DefaultValue)
                }
            }

            impl<T0> CmdPortalBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [CmdPortal].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdPortal>
                where
                    Self: ::planus::WriteAsOffset<CmdPortal>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAs<::planus::Offset<CmdPortal>> for CmdPortalBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<CmdPortal>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdPortal> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAsOptional<::planus::Offset<CmdPortal>> for CmdPortalBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<CmdPortal>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdPortal>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>> ::planus::WriteAsOffset<CmdPortal>
                for CmdPortalBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdPortal> {
                    let (v0,) = &self.0;
                    CmdPortal::create(builder, v0)
                }
            }

            /// Reference to a deserialized [CmdPortal].
            #[derive(Copy, Clone)]
            pub struct CmdPortalRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> CmdPortalRef<'a> {
                /// Getter for the [`portal` field](CmdPortal#structfield.portal).
                #[inline]
                pub fn portal(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "CmdPortal", "portal")?.unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for CmdPortalRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("CmdPortalRef");
                    f.field("portal", &self.portal());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<CmdPortalRef<'a>> for CmdPortal {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: CmdPortalRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        portal: ::core::convert::TryInto::try_into(value.portal()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for CmdPortalRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for CmdPortalRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[CmdPortalRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<CmdPortal>> for CmdPortal {
                type Value = ::planus::Offset<CmdPortal>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<CmdPortal>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for CmdPortalRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[CmdPortalRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `CmdEmpty` in the namespace `ingress.protocol`
            ///
            /// Generated from these locations:
            /// * Table `CmdEmpty` in the file `crates/arena-protocol/schemas/protocol.fbs:147`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct CmdEmpty {}

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for CmdEmpty {
                fn default() -> Self {
                    Self {}
                }
            }

            impl CmdEmpty {
                /// Creates a [CmdEmptyBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> CmdEmptyBuilder<()> {
                    CmdEmptyBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(builder: &mut ::planus::Builder) -> ::planus::Offset<Self> {
                    let table_writer: ::planus::table_writer::TableWriter<4> =
                        ::core::default::Default::default();
                    unsafe {
                        table_writer.finish(builder, |_table_writer| {});
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<CmdEmpty>> for CmdEmpty {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdEmpty> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<CmdEmpty>> for CmdEmpty {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdEmpty>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<CmdEmpty> for CmdEmpty {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdEmpty> {
                    CmdEmpty::create(builder)
                }
            }

            /// Builder for serializing an instance of the [CmdEmpty] type.
            ///
            /// Can be created using the [CmdEmpty::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct CmdEmptyBuilder<State>(State);

            impl CmdEmptyBuilder<()> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [CmdEmpty].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdEmpty>
                where
                    Self: ::planus::WriteAsOffset<CmdEmpty>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl ::planus::WriteAs<::planus::Offset<CmdEmpty>> for CmdEmptyBuilder<()> {
                type Prepared = ::planus::Offset<CmdEmpty>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdEmpty> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<CmdEmpty>> for CmdEmptyBuilder<()> {
                type Prepared = ::planus::Offset<CmdEmpty>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdEmpty>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<CmdEmpty> for CmdEmptyBuilder<()> {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdEmpty> {
                    CmdEmpty::create(builder)
                }
            }

            /// Reference to a deserialized [CmdEmpty].
            #[derive(Copy, Clone)]
            pub struct CmdEmptyRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> CmdEmptyRef<'a> {}

            impl<'a> ::core::fmt::Debug for CmdEmptyRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("CmdEmptyRef");

                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<CmdEmptyRef<'a>> for CmdEmpty {
                type Error = ::planus::Error;

                fn try_from(_value: CmdEmptyRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {})
                }
            }

            impl<'a> ::planus::TableRead<'a> for CmdEmptyRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for CmdEmptyRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[CmdEmptyRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<CmdEmpty>> for CmdEmpty {
                type Value = ::planus::Offset<CmdEmpty>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<CmdEmpty>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for CmdEmptyRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[CmdEmptyRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `CmdLink` in the namespace `ingress.protocol`
            ///
            /// Generated from these locations:
            /// * Table `CmdLink` in the file `crates/arena-protocol/schemas/protocol.fbs:148`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct CmdLink {
                /// The field `a` in the table `CmdLink`
                pub a: u32,
                /// The field `b` in the table `CmdLink`
                pub b: u32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for CmdLink {
                fn default() -> Self {
                    Self { a: 0, b: 0 }
                }
            }

            impl CmdLink {
                /// Creates a [CmdLinkBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> CmdLinkBuilder<()> {
                    CmdLinkBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_a: impl ::planus::WriteAsDefault<u32, u32>,
                    field_b: impl ::planus::WriteAsDefault<u32, u32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_a = field_a.prepare(builder, &0);
                    let prepared_b = field_b.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_a.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }
                    if prepared_b.is_some() {
                        table_writer.write_entry::<u32>(1);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_a) = prepared_a {
                                object_writer.write::<_, _, 4>(&prepared_a);
                            }
                            if let ::core::option::Option::Some(prepared_b) = prepared_b {
                                object_writer.write::<_, _, 4>(&prepared_b);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<CmdLink>> for CmdLink {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdLink> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<CmdLink>> for CmdLink {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdLink>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<CmdLink> for CmdLink {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdLink> {
                    CmdLink::create(builder, self.a, self.b)
                }
            }

            /// Builder for serializing an instance of the [CmdLink] type.
            ///
            /// Can be created using the [CmdLink::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct CmdLinkBuilder<State>(State);

            impl CmdLinkBuilder<()> {
                /// Setter for the [`a` field](CmdLink#structfield.a).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn a<T0>(self, value: T0) -> CmdLinkBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    CmdLinkBuilder((value,))
                }

                /// Sets the [`a` field](CmdLink#structfield.a) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn a_as_default(self) -> CmdLinkBuilder<(::planus::DefaultValue,)> {
                    self.a(::planus::DefaultValue)
                }
            }

            impl<T0> CmdLinkBuilder<(T0,)> {
                /// Setter for the [`b` field](CmdLink#structfield.b).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn b<T1>(self, value: T1) -> CmdLinkBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0,) = self.0;
                    CmdLinkBuilder((v0, value))
                }

                /// Sets the [`b` field](CmdLink#structfield.b) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn b_as_default(self) -> CmdLinkBuilder<(T0, ::planus::DefaultValue)> {
                    self.b(::planus::DefaultValue)
                }
            }

            impl<T0, T1> CmdLinkBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [CmdLink].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdLink>
                where
                    Self: ::planus::WriteAsOffset<CmdLink>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>, T1: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAs<::planus::Offset<CmdLink>> for CmdLinkBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<CmdLink>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdLink> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>, T1: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAsOptional<::planus::Offset<CmdLink>> for CmdLinkBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<CmdLink>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdLink>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>, T1: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAsOffset<CmdLink> for CmdLinkBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CmdLink> {
                    let (v0, v1) = &self.0;
                    CmdLink::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [CmdLink].
            #[derive(Copy, Clone)]
            pub struct CmdLinkRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> CmdLinkRef<'a> {
                /// Getter for the [`a` field](CmdLink#structfield.a).
                #[inline]
                pub fn a(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(self.0.access(0, "CmdLink", "a")?.unwrap_or(0))
                }

                /// Getter for the [`b` field](CmdLink#structfield.b).
                #[inline]
                pub fn b(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(self.0.access(1, "CmdLink", "b")?.unwrap_or(0))
                }
            }

            impl<'a> ::core::fmt::Debug for CmdLinkRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("CmdLinkRef");
                    f.field("a", &self.a());
                    f.field("b", &self.b());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<CmdLinkRef<'a>> for CmdLink {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: CmdLinkRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        a: ::core::convert::TryInto::try_into(value.a()?)?,
                        b: ::core::convert::TryInto::try_into(value.b()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for CmdLinkRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for CmdLinkRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[CmdLinkRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<CmdLink>> for CmdLink {
                type Value = ::planus::Offset<CmdLink>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<CmdLink>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for CmdLinkRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[CmdLinkRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `KeyCount` in the namespace `ingress.protocol`
            ///
            /// Generated from these locations:
            /// * Table `KeyCount` in the file `crates/arena-protocol/schemas/protocol.fbs:149`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct KeyCount {
                /// The field `portal` in the table `KeyCount`
                pub portal: u32,
                /// The field `count` in the table `KeyCount`
                pub count: u32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for KeyCount {
                fn default() -> Self {
                    Self {
                        portal: 0,
                        count: 0,
                    }
                }
            }

            impl KeyCount {
                /// Creates a [KeyCountBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> KeyCountBuilder<()> {
                    KeyCountBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_portal: impl ::planus::WriteAsDefault<u32, u32>,
                    field_count: impl ::planus::WriteAsDefault<u32, u32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_portal = field_portal.prepare(builder, &0);
                    let prepared_count = field_count.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_portal.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }
                    if prepared_count.is_some() {
                        table_writer.write_entry::<u32>(1);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_portal) = prepared_portal {
                                object_writer.write::<_, _, 4>(&prepared_portal);
                            }
                            if let ::core::option::Option::Some(prepared_count) = prepared_count {
                                object_writer.write::<_, _, 4>(&prepared_count);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<KeyCount>> for KeyCount {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<KeyCount> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<KeyCount>> for KeyCount {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<KeyCount>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<KeyCount> for KeyCount {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<KeyCount> {
                    KeyCount::create(builder, self.portal, self.count)
                }
            }

            /// Builder for serializing an instance of the [KeyCount] type.
            ///
            /// Can be created using the [KeyCount::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct KeyCountBuilder<State>(State);

            impl KeyCountBuilder<()> {
                /// Setter for the [`portal` field](KeyCount#structfield.portal).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal<T0>(self, value: T0) -> KeyCountBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    KeyCountBuilder((value,))
                }

                /// Sets the [`portal` field](KeyCount#structfield.portal) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn portal_as_default(self) -> KeyCountBuilder<(::planus::DefaultValue,)> {
                    self.portal(::planus::DefaultValue)
                }
            }

            impl<T0> KeyCountBuilder<(T0,)> {
                /// Setter for the [`count` field](KeyCount#structfield.count).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn count<T1>(self, value: T1) -> KeyCountBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0,) = self.0;
                    KeyCountBuilder((v0, value))
                }

                /// Sets the [`count` field](KeyCount#structfield.count) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn count_as_default(self) -> KeyCountBuilder<(T0, ::planus::DefaultValue)> {
                    self.count(::planus::DefaultValue)
                }
            }

            impl<T0, T1> KeyCountBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [KeyCount].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<KeyCount>
                where
                    Self: ::planus::WriteAsOffset<KeyCount>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>, T1: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAs<::planus::Offset<KeyCount>> for KeyCountBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<KeyCount>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<KeyCount> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>, T1: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAsOptional<::planus::Offset<KeyCount>>
                for KeyCountBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<KeyCount>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<KeyCount>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>, T1: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAsOffset<KeyCount> for KeyCountBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<KeyCount> {
                    let (v0, v1) = &self.0;
                    KeyCount::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [KeyCount].
            #[derive(Copy, Clone)]
            pub struct KeyCountRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> KeyCountRef<'a> {
                /// Getter for the [`portal` field](KeyCount#structfield.portal).
                #[inline]
                pub fn portal(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(self.0.access(0, "KeyCount", "portal")?.unwrap_or(0))
                }

                /// Getter for the [`count` field](KeyCount#structfield.count).
                #[inline]
                pub fn count(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(self.0.access(1, "KeyCount", "count")?.unwrap_or(0))
                }
            }

            impl<'a> ::core::fmt::Debug for KeyCountRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("KeyCountRef");
                    f.field("portal", &self.portal());
                    f.field("count", &self.count());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<KeyCountRef<'a>> for KeyCount {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: KeyCountRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        portal: ::core::convert::TryInto::try_into(value.portal()?)?,
                        count: ::core::convert::TryInto::try_into(value.count()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for KeyCountRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for KeyCountRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[KeyCountRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<KeyCount>> for KeyCount {
                type Value = ::planus::Offset<KeyCount>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<KeyCount>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for KeyCountRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[KeyCountRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `CmdTransfer` in the namespace `ingress.protocol`
            ///
            /// Generated from these locations:
            /// * Table `CmdTransfer` in the file `crates/arena-protocol/schemas/protocol.fbs:150`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct CmdTransfer {
                /// The field `target_agent` in the table `CmdTransfer`
                pub target_agent: u32,
                /// The field `supply` in the table `CmdTransfer`
                pub supply: u32,
                /// The field `keys` in the table `CmdTransfer`
                pub keys: ::core::option::Option<::planus::alloc::vec::Vec<self::KeyCount>>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for CmdTransfer {
                fn default() -> Self {
                    Self {
                        target_agent: 0,
                        supply: 0,
                        keys: ::core::default::Default::default(),
                    }
                }
            }

            impl CmdTransfer {
                /// Creates a [CmdTransferBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> CmdTransferBuilder<()> {
                    CmdTransferBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_target_agent: impl ::planus::WriteAsDefault<u32, u32>,
                    field_supply: impl ::planus::WriteAsDefault<u32, u32>,
                    field_keys: impl ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::KeyCount>]>,
                    >,
                ) -> ::planus::Offset<Self> {
                    let prepared_target_agent = field_target_agent.prepare(builder, &0);
                    let prepared_supply = field_supply.prepare(builder, &0);
                    let prepared_keys = field_keys.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<10> =
                        ::core::default::Default::default();
                    if prepared_target_agent.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }
                    if prepared_supply.is_some() {
                        table_writer.write_entry::<u32>(1);
                    }
                    if prepared_keys.is_some() {
                        table_writer
                            .write_entry::<::planus::Offset<[::planus::Offset<self::KeyCount>]>>(2);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_target_agent) =
                                prepared_target_agent
                            {
                                object_writer.write::<_, _, 4>(&prepared_target_agent);
                            }
                            if let ::core::option::Option::Some(prepared_supply) = prepared_supply {
                                object_writer.write::<_, _, 4>(&prepared_supply);
                            }
                            if let ::core::option::Option::Some(prepared_keys) = prepared_keys {
                                object_writer.write::<_, _, 4>(&prepared_keys);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<CmdTransfer>> for CmdTransfer {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CmdTransfer> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<CmdTransfer>> for CmdTransfer {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdTransfer>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<CmdTransfer> for CmdTransfer {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CmdTransfer> {
                    CmdTransfer::create(builder, self.target_agent, self.supply, &self.keys)
                }
            }

            /// Builder for serializing an instance of the [CmdTransfer] type.
            ///
            /// Can be created using the [CmdTransfer::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct CmdTransferBuilder<State>(State);

            impl CmdTransferBuilder<()> {
                /// Setter for the [`target_agent` field](CmdTransfer#structfield.target_agent).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn target_agent<T0>(self, value: T0) -> CmdTransferBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    CmdTransferBuilder((value,))
                }

                /// Sets the [`target_agent` field](CmdTransfer#structfield.target_agent) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn target_agent_as_default(
                    self,
                ) -> CmdTransferBuilder<(::planus::DefaultValue,)> {
                    self.target_agent(::planus::DefaultValue)
                }
            }

            impl<T0> CmdTransferBuilder<(T0,)> {
                /// Setter for the [`supply` field](CmdTransfer#structfield.supply).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn supply<T1>(self, value: T1) -> CmdTransferBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0,) = self.0;
                    CmdTransferBuilder((v0, value))
                }

                /// Sets the [`supply` field](CmdTransfer#structfield.supply) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn supply_as_default(self) -> CmdTransferBuilder<(T0, ::planus::DefaultValue)> {
                    self.supply(::planus::DefaultValue)
                }
            }

            impl<T0, T1> CmdTransferBuilder<(T0, T1)> {
                /// Setter for the [`keys` field](CmdTransfer#structfield.keys).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn keys<T2>(self, value: T2) -> CmdTransferBuilder<(T0, T1, T2)>
                where
                    T2: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::KeyCount>]>,
                        >,
                {
                    let (v0, v1) = self.0;
                    CmdTransferBuilder((v0, v1, value))
                }

                /// Sets the [`keys` field](CmdTransfer#structfield.keys) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn keys_as_null(self) -> CmdTransferBuilder<(T0, T1, ())> {
                    self.keys(())
                }
            }

            impl<T0, T1, T2> CmdTransferBuilder<(T0, T1, T2)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [CmdTransfer].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CmdTransfer>
                where
                    Self: ::planus::WriteAsOffset<CmdTransfer>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::KeyCount>]>>,
            > ::planus::WriteAs<::planus::Offset<CmdTransfer>>
                for CmdTransferBuilder<(T0, T1, T2)>
            {
                type Prepared = ::planus::Offset<CmdTransfer>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CmdTransfer> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::KeyCount>]>>,
            > ::planus::WriteAsOptional<::planus::Offset<CmdTransfer>>
                for CmdTransferBuilder<(T0, T1, T2)>
            {
                type Prepared = ::planus::Offset<CmdTransfer>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CmdTransfer>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::KeyCount>]>>,
            > ::planus::WriteAsOffset<CmdTransfer> for CmdTransferBuilder<(T0, T1, T2)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CmdTransfer> {
                    let (v0, v1, v2) = &self.0;
                    CmdTransfer::create(builder, v0, v1, v2)
                }
            }

            /// Reference to a deserialized [CmdTransfer].
            #[derive(Copy, Clone)]
            pub struct CmdTransferRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> CmdTransferRef<'a> {
                /// Getter for the [`target_agent` field](CmdTransfer#structfield.target_agent).
                #[inline]
                pub fn target_agent(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(0, "CmdTransfer", "target_agent")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`supply` field](CmdTransfer#structfield.supply).
                #[inline]
                pub fn supply(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(1, "CmdTransfer", "supply")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`keys` field](CmdTransfer#structfield.keys).
                #[inline]
                pub fn keys(
                    &self,
                ) -> ::planus::Result<
                    ::core::option::Option<
                        ::planus::Vector<'a, ::planus::Result<self::KeyCountRef<'a>>>,
                    >,
                > {
                    self.0.access(2, "CmdTransfer", "keys")
                }
            }

            impl<'a> ::core::fmt::Debug for CmdTransferRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("CmdTransferRef");
                    f.field("target_agent", &self.target_agent());
                    f.field("supply", &self.supply());
                    if let ::core::option::Option::Some(field_keys) = self.keys().transpose() {
                        f.field("keys", &field_keys);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<CmdTransferRef<'a>> for CmdTransfer {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: CmdTransferRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        target_agent: ::core::convert::TryInto::try_into(value.target_agent()?)?,
                        supply: ::core::convert::TryInto::try_into(value.supply()?)?,
                        keys: if let ::core::option::Option::Some(keys) = value.keys()? {
                            ::core::option::Option::Some(keys.to_vec_result()?)
                        } else {
                            ::core::option::Option::None
                        },
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for CmdTransferRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for CmdTransferRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[CmdTransferRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<CmdTransfer>> for CmdTransfer {
                type Value = ::planus::Offset<CmdTransfer>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<CmdTransfer>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for CmdTransferRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[CmdTransferRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `Command` in the namespace `ingress.protocol`
            ///
            /// Generated from these locations:
            /// * Table `Command` in the file `crates/arena-protocol/schemas/protocol.fbs:156`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct Command {
                /// The field `body` in the table `Command`
                pub body: ::core::option::Option<self::CommandBody>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for Command {
                fn default() -> Self {
                    Self {
                        body: ::core::default::Default::default(),
                    }
                }
            }

            impl Command {
                /// Creates a [CommandBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> CommandBuilder<()> {
                    CommandBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_body: impl ::planus::WriteAsOptionalUnion<self::CommandBody>,
                ) -> ::planus::Offset<Self> {
                    let prepared_body = field_body.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_body.is_some() {
                        table_writer.write_entry::<::planus::Offset<self::CommandBody>>(1);
                    }
                    if prepared_body.is_some() {
                        table_writer.write_entry::<u8>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
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

            impl ::planus::WriteAs<::planus::Offset<Command>> for Command {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Command> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<Command>> for Command {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<Command>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<Command> for Command {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Command> {
                    Command::create(builder, &self.body)
                }
            }

            /// Builder for serializing an instance of the [Command] type.
            ///
            /// Can be created using the [Command::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct CommandBuilder<State>(State);

            impl CommandBuilder<()> {
                /// Setter for the [`body` field](Command#structfield.body).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn body<T0>(self, value: T0) -> CommandBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsOptionalUnion<self::CommandBody>,
                {
                    CommandBuilder((value,))
                }

                /// Sets the [`body` field](Command#structfield.body) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn body_as_null(self) -> CommandBuilder<((),)> {
                    self.body(())
                }
            }

            impl<T0> CommandBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [Command].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<Command>
                where
                    Self: ::planus::WriteAsOffset<Command>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsOptionalUnion<self::CommandBody>>
                ::planus::WriteAs<::planus::Offset<Command>> for CommandBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<Command>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Command> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsOptionalUnion<self::CommandBody>>
                ::planus::WriteAsOptional<::planus::Offset<Command>> for CommandBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<Command>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<Command>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsOptionalUnion<self::CommandBody>>
                ::planus::WriteAsOffset<Command> for CommandBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Command> {
                    let (v0,) = &self.0;
                    Command::create(builder, v0)
                }
            }

            /// Reference to a deserialized [Command].
            #[derive(Copy, Clone)]
            pub struct CommandRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> CommandRef<'a> {
                /// Getter for the [`body` field](Command#structfield.body).
                #[inline]
                pub fn body(
                    &self,
                ) -> ::planus::Result<::core::option::Option<self::CommandBodyRef<'a>>>
                {
                    self.0.access_union(0, "Command", "body")
                }
            }

            impl<'a> ::core::fmt::Debug for CommandRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("CommandRef");
                    if let ::core::option::Option::Some(field_body) = self.body().transpose() {
                        f.field("body", &field_body);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<CommandRef<'a>> for Command {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: CommandRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        body: if let ::core::option::Option::Some(body) = value.body()? {
                            ::core::option::Option::Some(::core::convert::TryInto::try_into(body)?)
                        } else {
                            ::core::option::Option::None
                        },
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for CommandRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for CommandRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[CommandRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<Command>> for Command {
                type Value = ::planus::Offset<Command>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<Command>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for CommandRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[CommandRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `SubmitCommandStream` in the namespace `ingress.protocol`
            ///
            /// Generated from these locations:
            /// * Table `SubmitCommandStream` in the file `crates/arena-protocol/schemas/protocol.fbs:158`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct SubmitCommandStream {
                /// The field `round` in the table `SubmitCommandStream`
                pub round: u32,
                /// The field `agent_id` in the table `SubmitCommandStream`
                pub agent_id: u32,
                /// The field `commands` in the table `SubmitCommandStream`
                pub commands: ::core::option::Option<::planus::alloc::vec::Vec<self::Command>>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for SubmitCommandStream {
                fn default() -> Self {
                    Self {
                        round: 0,
                        agent_id: 0,
                        commands: ::core::default::Default::default(),
                    }
                }
            }

            impl SubmitCommandStream {
                /// Creates a [SubmitCommandStreamBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> SubmitCommandStreamBuilder<()> {
                    SubmitCommandStreamBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_round: impl ::planus::WriteAsDefault<u32, u32>,
                    field_agent_id: impl ::planus::WriteAsDefault<u32, u32>,
                    field_commands: impl ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::Command>]>,
                    >,
                ) -> ::planus::Offset<Self> {
                    let prepared_round = field_round.prepare(builder, &0);
                    let prepared_agent_id = field_agent_id.prepare(builder, &0);
                    let prepared_commands = field_commands.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<10> =
                        ::core::default::Default::default();
                    if prepared_round.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }
                    if prepared_agent_id.is_some() {
                        table_writer.write_entry::<u32>(1);
                    }
                    if prepared_commands.is_some() {
                        table_writer
                            .write_entry::<::planus::Offset<[::planus::Offset<self::Command>]>>(2);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_round) = prepared_round {
                                object_writer.write::<_, _, 4>(&prepared_round);
                            }
                            if let ::core::option::Option::Some(prepared_agent_id) =
                                prepared_agent_id
                            {
                                object_writer.write::<_, _, 4>(&prepared_agent_id);
                            }
                            if let ::core::option::Option::Some(prepared_commands) =
                                prepared_commands
                            {
                                object_writer.write::<_, _, 4>(&prepared_commands);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<SubmitCommandStream>> for SubmitCommandStream {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitCommandStream> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<SubmitCommandStream>> for SubmitCommandStream {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SubmitCommandStream>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<SubmitCommandStream> for SubmitCommandStream {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitCommandStream> {
                    SubmitCommandStream::create(builder, self.round, self.agent_id, &self.commands)
                }
            }

            /// Builder for serializing an instance of the [SubmitCommandStream] type.
            ///
            /// Can be created using the [SubmitCommandStream::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct SubmitCommandStreamBuilder<State>(State);

            impl SubmitCommandStreamBuilder<()> {
                /// Setter for the [`round` field](SubmitCommandStream#structfield.round).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn round<T0>(self, value: T0) -> SubmitCommandStreamBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    SubmitCommandStreamBuilder((value,))
                }

                /// Sets the [`round` field](SubmitCommandStream#structfield.round) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn round_as_default(
                    self,
                ) -> SubmitCommandStreamBuilder<(::planus::DefaultValue,)> {
                    self.round(::planus::DefaultValue)
                }
            }

            impl<T0> SubmitCommandStreamBuilder<(T0,)> {
                /// Setter for the [`agent_id` field](SubmitCommandStream#structfield.agent_id).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn agent_id<T1>(self, value: T1) -> SubmitCommandStreamBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0,) = self.0;
                    SubmitCommandStreamBuilder((v0, value))
                }

                /// Sets the [`agent_id` field](SubmitCommandStream#structfield.agent_id) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn agent_id_as_default(
                    self,
                ) -> SubmitCommandStreamBuilder<(T0, ::planus::DefaultValue)> {
                    self.agent_id(::planus::DefaultValue)
                }
            }

            impl<T0, T1> SubmitCommandStreamBuilder<(T0, T1)> {
                /// Setter for the [`commands` field](SubmitCommandStream#structfield.commands).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn commands<T2>(self, value: T2) -> SubmitCommandStreamBuilder<(T0, T1, T2)>
                where
                    T2: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::Command>]>,
                        >,
                {
                    let (v0, v1) = self.0;
                    SubmitCommandStreamBuilder((v0, v1, value))
                }

                /// Sets the [`commands` field](SubmitCommandStream#structfield.commands) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn commands_as_null(self) -> SubmitCommandStreamBuilder<(T0, T1, ())> {
                    self.commands(())
                }
            }

            impl<T0, T1, T2> SubmitCommandStreamBuilder<(T0, T1, T2)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [SubmitCommandStream].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitCommandStream>
                where
                    Self: ::planus::WriteAsOffset<SubmitCommandStream>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::Command>]>>,
            > ::planus::WriteAs<::planus::Offset<SubmitCommandStream>>
                for SubmitCommandStreamBuilder<(T0, T1, T2)>
            {
                type Prepared = ::planus::Offset<SubmitCommandStream>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitCommandStream> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::Command>]>>,
            > ::planus::WriteAsOptional<::planus::Offset<SubmitCommandStream>>
                for SubmitCommandStreamBuilder<(T0, T1, T2)>
            {
                type Prepared = ::planus::Offset<SubmitCommandStream>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SubmitCommandStream>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::Command>]>>,
            > ::planus::WriteAsOffset<SubmitCommandStream>
                for SubmitCommandStreamBuilder<(T0, T1, T2)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmitCommandStream> {
                    let (v0, v1, v2) = &self.0;
                    SubmitCommandStream::create(builder, v0, v1, v2)
                }
            }

            /// Reference to a deserialized [SubmitCommandStream].
            #[derive(Copy, Clone)]
            pub struct SubmitCommandStreamRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> SubmitCommandStreamRef<'a> {
                /// Getter for the [`round` field](SubmitCommandStream#structfield.round).
                #[inline]
                pub fn round(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(0, "SubmitCommandStream", "round")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`agent_id` field](SubmitCommandStream#structfield.agent_id).
                #[inline]
                pub fn agent_id(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(1, "SubmitCommandStream", "agent_id")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`commands` field](SubmitCommandStream#structfield.commands).
                #[inline]
                pub fn commands(
                    &self,
                ) -> ::planus::Result<
                    ::core::option::Option<
                        ::planus::Vector<'a, ::planus::Result<self::CommandRef<'a>>>,
                    >,
                > {
                    self.0.access(2, "SubmitCommandStream", "commands")
                }
            }

            impl<'a> ::core::fmt::Debug for SubmitCommandStreamRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("SubmitCommandStreamRef");
                    f.field("round", &self.round());
                    f.field("agent_id", &self.agent_id());
                    if let ::core::option::Option::Some(field_commands) =
                        self.commands().transpose()
                    {
                        f.field("commands", &field_commands);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<SubmitCommandStreamRef<'a>> for SubmitCommandStream {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: SubmitCommandStreamRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        round: ::core::convert::TryInto::try_into(value.round()?)?,
                        agent_id: ::core::convert::TryInto::try_into(value.agent_id()?)?,
                        commands: if let ::core::option::Option::Some(commands) =
                            value.commands()?
                        {
                            ::core::option::Option::Some(commands.to_vec_result()?)
                        } else {
                            ::core::option::Option::None
                        },
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for SubmitCommandStreamRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for SubmitCommandStreamRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[SubmitCommandStreamRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<SubmitCommandStream>> for SubmitCommandStream {
                type Value = ::planus::Offset<SubmitCommandStream>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<SubmitCommandStream>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for SubmitCommandStreamRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[SubmitCommandStreamRef]",
                            "read_as_root",
                            0,
                        )
                    })
                }
            }

            ///  Server -> Client reply to SubmitCommandStream. status: 0 = accepted
            ///  (first stream, slot frozen), 1 = rejected structural (slot still frozen,
            ///  ADR-0004), 2 = ignored duplicate, 3 = envelope-unlocatable (wrong round /
            ///  unknown agent / faction mismatch; nothing frozen), 4 = dropped late
            ///  (completed at/after the deadline; nothing frozen).
            ///
            /// Generated from these locations:
            /// * Table `SubmissionResult` in the file `crates/arena-protocol/schemas/protocol.fbs:169`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct SubmissionResult {
                /// The field `round` in the table `SubmissionResult`
                pub round: u32,
                /// The field `agent_id` in the table `SubmissionResult`
                pub agent_id: u32,
                /// The field `status` in the table `SubmissionResult`
                pub status: u8,
                /// The field `detail` in the table `SubmissionResult`
                pub detail: ::core::option::Option<::planus::alloc::string::String>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for SubmissionResult {
                fn default() -> Self {
                    Self {
                        round: 0,
                        agent_id: 0,
                        status: 0,
                        detail: ::core::default::Default::default(),
                    }
                }
            }

            impl SubmissionResult {
                /// Creates a [SubmissionResultBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> SubmissionResultBuilder<()> {
                    SubmissionResultBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_round: impl ::planus::WriteAsDefault<u32, u32>,
                    field_agent_id: impl ::planus::WriteAsDefault<u32, u32>,
                    field_status: impl ::planus::WriteAsDefault<u8, u8>,
                    field_detail: impl ::planus::WriteAsOptional<
                        ::planus::Offset<::core::primitive::str>,
                    >,
                ) -> ::planus::Offset<Self> {
                    let prepared_round = field_round.prepare(builder, &0);
                    let prepared_agent_id = field_agent_id.prepare(builder, &0);
                    let prepared_status = field_status.prepare(builder, &0);
                    let prepared_detail = field_detail.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<12> =
                        ::core::default::Default::default();
                    if prepared_round.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }
                    if prepared_agent_id.is_some() {
                        table_writer.write_entry::<u32>(1);
                    }
                    if prepared_detail.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(3);
                    }
                    if prepared_status.is_some() {
                        table_writer.write_entry::<u8>(2);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_round) = prepared_round {
                                object_writer.write::<_, _, 4>(&prepared_round);
                            }
                            if let ::core::option::Option::Some(prepared_agent_id) =
                                prepared_agent_id
                            {
                                object_writer.write::<_, _, 4>(&prepared_agent_id);
                            }
                            if let ::core::option::Option::Some(prepared_detail) = prepared_detail {
                                object_writer.write::<_, _, 4>(&prepared_detail);
                            }
                            if let ::core::option::Option::Some(prepared_status) = prepared_status {
                                object_writer.write::<_, _, 1>(&prepared_status);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<SubmissionResult>> for SubmissionResult {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmissionResult> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<SubmissionResult>> for SubmissionResult {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SubmissionResult>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<SubmissionResult> for SubmissionResult {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmissionResult> {
                    SubmissionResult::create(
                        builder,
                        self.round,
                        self.agent_id,
                        self.status,
                        &self.detail,
                    )
                }
            }

            /// Builder for serializing an instance of the [SubmissionResult] type.
            ///
            /// Can be created using the [SubmissionResult::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct SubmissionResultBuilder<State>(State);

            impl SubmissionResultBuilder<()> {
                /// Setter for the [`round` field](SubmissionResult#structfield.round).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn round<T0>(self, value: T0) -> SubmissionResultBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    SubmissionResultBuilder((value,))
                }

                /// Sets the [`round` field](SubmissionResult#structfield.round) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn round_as_default(
                    self,
                ) -> SubmissionResultBuilder<(::planus::DefaultValue,)> {
                    self.round(::planus::DefaultValue)
                }
            }

            impl<T0> SubmissionResultBuilder<(T0,)> {
                /// Setter for the [`agent_id` field](SubmissionResult#structfield.agent_id).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn agent_id<T1>(self, value: T1) -> SubmissionResultBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0,) = self.0;
                    SubmissionResultBuilder((v0, value))
                }

                /// Sets the [`agent_id` field](SubmissionResult#structfield.agent_id) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn agent_id_as_default(
                    self,
                ) -> SubmissionResultBuilder<(T0, ::planus::DefaultValue)> {
                    self.agent_id(::planus::DefaultValue)
                }
            }

            impl<T0, T1> SubmissionResultBuilder<(T0, T1)> {
                /// Setter for the [`status` field](SubmissionResult#structfield.status).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn status<T2>(self, value: T2) -> SubmissionResultBuilder<(T0, T1, T2)>
                where
                    T2: ::planus::WriteAsDefault<u8, u8>,
                {
                    let (v0, v1) = self.0;
                    SubmissionResultBuilder((v0, v1, value))
                }

                /// Sets the [`status` field](SubmissionResult#structfield.status) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn status_as_default(
                    self,
                ) -> SubmissionResultBuilder<(T0, T1, ::planus::DefaultValue)> {
                    self.status(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2> SubmissionResultBuilder<(T0, T1, T2)> {
                /// Setter for the [`detail` field](SubmissionResult#structfield.detail).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn detail<T3>(self, value: T3) -> SubmissionResultBuilder<(T0, T1, T2, T3)>
                where
                    T3: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    let (v0, v1, v2) = self.0;
                    SubmissionResultBuilder((v0, v1, v2, value))
                }

                /// Sets the [`detail` field](SubmissionResult#structfield.detail) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn detail_as_null(self) -> SubmissionResultBuilder<(T0, T1, T2, ())> {
                    self.detail(())
                }
            }

            impl<T0, T1, T2, T3> SubmissionResultBuilder<(T0, T1, T2, T3)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [SubmissionResult].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmissionResult>
                where
                    Self: ::planus::WriteAsOffset<SubmissionResult>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsDefault<u8, u8>,
                T3: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
            > ::planus::WriteAs<::planus::Offset<SubmissionResult>>
                for SubmissionResultBuilder<(T0, T1, T2, T3)>
            {
                type Prepared = ::planus::Offset<SubmissionResult>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmissionResult> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsDefault<u8, u8>,
                T3: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
            > ::planus::WriteAsOptional<::planus::Offset<SubmissionResult>>
                for SubmissionResultBuilder<(T0, T1, T2, T3)>
            {
                type Prepared = ::planus::Offset<SubmissionResult>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SubmissionResult>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u32, u32>,
                T1: ::planus::WriteAsDefault<u32, u32>,
                T2: ::planus::WriteAsDefault<u8, u8>,
                T3: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
            > ::planus::WriteAsOffset<SubmissionResult>
                for SubmissionResultBuilder<(T0, T1, T2, T3)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<SubmissionResult> {
                    let (v0, v1, v2, v3) = &self.0;
                    SubmissionResult::create(builder, v0, v1, v2, v3)
                }
            }

            /// Reference to a deserialized [SubmissionResult].
            #[derive(Copy, Clone)]
            pub struct SubmissionResultRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> SubmissionResultRef<'a> {
                /// Getter for the [`round` field](SubmissionResult#structfield.round).
                #[inline]
                pub fn round(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "SubmissionResult", "round")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`agent_id` field](SubmissionResult#structfield.agent_id).
                #[inline]
                pub fn agent_id(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(1, "SubmissionResult", "agent_id")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`status` field](SubmissionResult#structfield.status).
                #[inline]
                pub fn status(&self) -> ::planus::Result<u8> {
                    ::core::result::Result::Ok(
                        self.0.access(2, "SubmissionResult", "status")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`detail` field](SubmissionResult#structfield.detail).
                #[inline]
                pub fn detail(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(3, "SubmissionResult", "detail")
                }
            }

            impl<'a> ::core::fmt::Debug for SubmissionResultRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("SubmissionResultRef");
                    f.field("round", &self.round());
                    f.field("agent_id", &self.agent_id());
                    f.field("status", &self.status());
                    if let ::core::option::Option::Some(field_detail) = self.detail().transpose() {
                        f.field("detail", &field_detail);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<SubmissionResultRef<'a>> for SubmissionResult {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: SubmissionResultRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        round: ::core::convert::TryInto::try_into(value.round()?)?,
                        agent_id: ::core::convert::TryInto::try_into(value.agent_id()?)?,
                        status: ::core::convert::TryInto::try_into(value.status()?)?,
                        detail: value.detail()?.map(::core::convert::Into::into),
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for SubmissionResultRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for SubmissionResultRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[SubmissionResultRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<SubmissionResult>> for SubmissionResult {
                type Value = ::planus::Offset<SubmissionResult>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<SubmissionResult>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for SubmissionResultRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[SubmissionResultRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Client -> Server: irreversibly freezes this faction's not-yet-submitted
            ///  agents as explicit passes for `round`.
            ///
            /// Generated from these locations:
            /// * Table `CommitRound` in the file `crates/arena-protocol/schemas/protocol.fbs:178`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct CommitRound {
                /// The field `round` in the table `CommitRound`
                pub round: u32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for CommitRound {
                fn default() -> Self {
                    Self { round: 0 }
                }
            }

            impl CommitRound {
                /// Creates a [CommitRoundBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> CommitRoundBuilder<()> {
                    CommitRoundBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_round: impl ::planus::WriteAsDefault<u32, u32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_round = field_round.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_round.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_round) = prepared_round {
                                object_writer.write::<_, _, 4>(&prepared_round);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<CommitRound>> for CommitRound {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CommitRound> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<CommitRound>> for CommitRound {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CommitRound>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<CommitRound> for CommitRound {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CommitRound> {
                    CommitRound::create(builder, self.round)
                }
            }

            /// Builder for serializing an instance of the [CommitRound] type.
            ///
            /// Can be created using the [CommitRound::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct CommitRoundBuilder<State>(State);

            impl CommitRoundBuilder<()> {
                /// Setter for the [`round` field](CommitRound#structfield.round).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn round<T0>(self, value: T0) -> CommitRoundBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    CommitRoundBuilder((value,))
                }

                /// Sets the [`round` field](CommitRound#structfield.round) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn round_as_default(self) -> CommitRoundBuilder<(::planus::DefaultValue,)> {
                    self.round(::planus::DefaultValue)
                }
            }

            impl<T0> CommitRoundBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [CommitRound].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CommitRound>
                where
                    Self: ::planus::WriteAsOffset<CommitRound>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAs<::planus::Offset<CommitRound>> for CommitRoundBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<CommitRound>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CommitRound> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAsOptional<::planus::Offset<CommitRound>>
                for CommitRoundBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<CommitRound>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CommitRound>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>> ::planus::WriteAsOffset<CommitRound>
                for CommitRoundBuilder<(T0,)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<CommitRound> {
                    let (v0,) = &self.0;
                    CommitRound::create(builder, v0)
                }
            }

            /// Reference to a deserialized [CommitRound].
            #[derive(Copy, Clone)]
            pub struct CommitRoundRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> CommitRoundRef<'a> {
                /// Getter for the [`round` field](CommitRound#structfield.round).
                #[inline]
                pub fn round(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "CommitRound", "round")?.unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for CommitRoundRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("CommitRoundRef");
                    f.field("round", &self.round());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<CommitRoundRef<'a>> for CommitRound {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: CommitRoundRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        round: ::core::convert::TryInto::try_into(value.round()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for CommitRoundRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for CommitRoundRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[CommitRoundRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<CommitRound>> for CommitRound {
                type Value = ::planus::Offset<CommitRound>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<CommitRound>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for CommitRoundRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[CommitRoundRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Server -> Client: input for `round` is closed; Settlement Phase begins
            ///  and SHM reads are forbidden until the next RoundStartSignal.
            ///
            /// Generated from these locations:
            /// * Table `RoundClosed` in the file `crates/arena-protocol/schemas/protocol.fbs:184`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct RoundClosed {
                /// The field `round` in the table `RoundClosed`
                pub round: u32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for RoundClosed {
                fn default() -> Self {
                    Self { round: 0 }
                }
            }

            impl RoundClosed {
                /// Creates a [RoundClosedBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> RoundClosedBuilder<()> {
                    RoundClosedBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_round: impl ::planus::WriteAsDefault<u32, u32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_round = field_round.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_round.is_some() {
                        table_writer.write_entry::<u32>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_round) = prepared_round {
                                object_writer.write::<_, _, 4>(&prepared_round);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<RoundClosed>> for RoundClosed {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundClosed> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<RoundClosed>> for RoundClosed {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<RoundClosed>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<RoundClosed> for RoundClosed {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundClosed> {
                    RoundClosed::create(builder, self.round)
                }
            }

            /// Builder for serializing an instance of the [RoundClosed] type.
            ///
            /// Can be created using the [RoundClosed::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct RoundClosedBuilder<State>(State);

            impl RoundClosedBuilder<()> {
                /// Setter for the [`round` field](RoundClosed#structfield.round).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn round<T0>(self, value: T0) -> RoundClosedBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u32, u32>,
                {
                    RoundClosedBuilder((value,))
                }

                /// Sets the [`round` field](RoundClosed#structfield.round) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn round_as_default(self) -> RoundClosedBuilder<(::planus::DefaultValue,)> {
                    self.round(::planus::DefaultValue)
                }
            }

            impl<T0> RoundClosedBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [RoundClosed].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundClosed>
                where
                    Self: ::planus::WriteAsOffset<RoundClosed>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAs<::planus::Offset<RoundClosed>> for RoundClosedBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<RoundClosed>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundClosed> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>>
                ::planus::WriteAsOptional<::planus::Offset<RoundClosed>>
                for RoundClosedBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<RoundClosed>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<RoundClosed>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<u32, u32>> ::planus::WriteAsOffset<RoundClosed>
                for RoundClosedBuilder<(T0,)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<RoundClosed> {
                    let (v0,) = &self.0;
                    RoundClosed::create(builder, v0)
                }
            }

            /// Reference to a deserialized [RoundClosed].
            #[derive(Copy, Clone)]
            pub struct RoundClosedRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> RoundClosedRef<'a> {
                /// Getter for the [`round` field](RoundClosed#structfield.round).
                #[inline]
                pub fn round(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "RoundClosed", "round")?.unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for RoundClosedRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("RoundClosedRef");
                    f.field("round", &self.round());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<RoundClosedRef<'a>> for RoundClosed {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: RoundClosedRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        round: ::core::convert::TryInto::try_into(value.round()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for RoundClosedRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for RoundClosedRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[RoundClosedRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<RoundClosed>> for RoundClosed {
                type Value = ::planus::Offset<RoundClosed>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<RoundClosed>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for RoundClosedRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[RoundClosedRef]", "read_as_root", 0)
                    })
                }
            }

            ///  Server -> Client, final frame. outcome: 0 EnlWin, 1 ResWin, 2 Draw,
            ///  3 SpawnForfeit, 4 ClientForfeit, 5 SetupError. Technical outcomes are
            ///  never dressed up as score results (CONTEXT Match Outcome).
            ///
            /// Generated from these locations:
            /// * Table `GameEnd` in the file `crates/arena-protocol/schemas/protocol.fbs:191`
            #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
            pub struct GameEnd {
                /// The field `outcome` in the table `GameEnd`
                pub outcome: u8,
                /// The field `detail` in the table `GameEnd`
                pub detail: ::core::option::Option<::planus::alloc::string::String>,
                /// The field `score_enl_hi` in the table `GameEnd`
                pub score_enl_hi: u64,
                /// The field `score_enl_lo` in the table `GameEnd`
                pub score_enl_lo: u64,
                /// The field `score_res_hi` in the table `GameEnd`
                pub score_res_hi: u64,
                /// The field `score_res_lo` in the table `GameEnd`
                pub score_res_lo: u64,
                /// The field `rounds_completed` in the table `GameEnd`
                pub rounds_completed: u32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for GameEnd {
                fn default() -> Self {
                    Self {
                        outcome: 0,
                        detail: ::core::default::Default::default(),
                        score_enl_hi: 0,
                        score_enl_lo: 0,
                        score_res_hi: 0,
                        score_res_lo: 0,
                        rounds_completed: 0,
                    }
                }
            }

            impl GameEnd {
                /// Creates a [GameEndBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> GameEndBuilder<()> {
                    GameEndBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_outcome: impl ::planus::WriteAsDefault<u8, u8>,
                    field_detail: impl ::planus::WriteAsOptional<
                        ::planus::Offset<::core::primitive::str>,
                    >,
                    field_score_enl_hi: impl ::planus::WriteAsDefault<u64, u64>,
                    field_score_enl_lo: impl ::planus::WriteAsDefault<u64, u64>,
                    field_score_res_hi: impl ::planus::WriteAsDefault<u64, u64>,
                    field_score_res_lo: impl ::planus::WriteAsDefault<u64, u64>,
                    field_rounds_completed: impl ::planus::WriteAsDefault<u32, u32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_outcome = field_outcome.prepare(builder, &0);
                    let prepared_detail = field_detail.prepare(builder);
                    let prepared_score_enl_hi = field_score_enl_hi.prepare(builder, &0);
                    let prepared_score_enl_lo = field_score_enl_lo.prepare(builder, &0);
                    let prepared_score_res_hi = field_score_res_hi.prepare(builder, &0);
                    let prepared_score_res_lo = field_score_res_lo.prepare(builder, &0);
                    let prepared_rounds_completed = field_rounds_completed.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<18> =
                        ::core::default::Default::default();
                    if prepared_score_enl_hi.is_some() {
                        table_writer.write_entry::<u64>(2);
                    }
                    if prepared_score_enl_lo.is_some() {
                        table_writer.write_entry::<u64>(3);
                    }
                    if prepared_score_res_hi.is_some() {
                        table_writer.write_entry::<u64>(4);
                    }
                    if prepared_score_res_lo.is_some() {
                        table_writer.write_entry::<u64>(5);
                    }
                    if prepared_detail.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(1);
                    }
                    if prepared_rounds_completed.is_some() {
                        table_writer.write_entry::<u32>(6);
                    }
                    if prepared_outcome.is_some() {
                        table_writer.write_entry::<u8>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
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
                            if let ::core::option::Option::Some(prepared_detail) = prepared_detail {
                                object_writer.write::<_, _, 4>(&prepared_detail);
                            }
                            if let ::core::option::Option::Some(prepared_rounds_completed) =
                                prepared_rounds_completed
                            {
                                object_writer.write::<_, _, 4>(&prepared_rounds_completed);
                            }
                            if let ::core::option::Option::Some(prepared_outcome) = prepared_outcome
                            {
                                object_writer.write::<_, _, 1>(&prepared_outcome);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<GameEnd>> for GameEnd {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<GameEnd> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<GameEnd>> for GameEnd {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<GameEnd>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<GameEnd> for GameEnd {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<GameEnd> {
                    GameEnd::create(
                        builder,
                        self.outcome,
                        &self.detail,
                        self.score_enl_hi,
                        self.score_enl_lo,
                        self.score_res_hi,
                        self.score_res_lo,
                        self.rounds_completed,
                    )
                }
            }

            /// Builder for serializing an instance of the [GameEnd] type.
            ///
            /// Can be created using the [GameEnd::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct GameEndBuilder<State>(State);

            impl GameEndBuilder<()> {
                /// Setter for the [`outcome` field](GameEnd#structfield.outcome).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn outcome<T0>(self, value: T0) -> GameEndBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u8, u8>,
                {
                    GameEndBuilder((value,))
                }

                /// Sets the [`outcome` field](GameEnd#structfield.outcome) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn outcome_as_default(self) -> GameEndBuilder<(::planus::DefaultValue,)> {
                    self.outcome(::planus::DefaultValue)
                }
            }

            impl<T0> GameEndBuilder<(T0,)> {
                /// Setter for the [`detail` field](GameEnd#structfield.detail).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn detail<T1>(self, value: T1) -> GameEndBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    let (v0,) = self.0;
                    GameEndBuilder((v0, value))
                }

                /// Sets the [`detail` field](GameEnd#structfield.detail) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn detail_as_null(self) -> GameEndBuilder<(T0, ())> {
                    self.detail(())
                }
            }

            impl<T0, T1> GameEndBuilder<(T0, T1)> {
                /// Setter for the [`score_enl_hi` field](GameEnd#structfield.score_enl_hi).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn score_enl_hi<T2>(self, value: T2) -> GameEndBuilder<(T0, T1, T2)>
                where
                    T2: ::planus::WriteAsDefault<u64, u64>,
                {
                    let (v0, v1) = self.0;
                    GameEndBuilder((v0, v1, value))
                }

                /// Sets the [`score_enl_hi` field](GameEnd#structfield.score_enl_hi) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn score_enl_hi_as_default(
                    self,
                ) -> GameEndBuilder<(T0, T1, ::planus::DefaultValue)> {
                    self.score_enl_hi(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2> GameEndBuilder<(T0, T1, T2)> {
                /// Setter for the [`score_enl_lo` field](GameEnd#structfield.score_enl_lo).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn score_enl_lo<T3>(self, value: T3) -> GameEndBuilder<(T0, T1, T2, T3)>
                where
                    T3: ::planus::WriteAsDefault<u64, u64>,
                {
                    let (v0, v1, v2) = self.0;
                    GameEndBuilder((v0, v1, v2, value))
                }

                /// Sets the [`score_enl_lo` field](GameEnd#structfield.score_enl_lo) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn score_enl_lo_as_default(
                    self,
                ) -> GameEndBuilder<(T0, T1, T2, ::planus::DefaultValue)> {
                    self.score_enl_lo(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3> GameEndBuilder<(T0, T1, T2, T3)> {
                /// Setter for the [`score_res_hi` field](GameEnd#structfield.score_res_hi).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn score_res_hi<T4>(self, value: T4) -> GameEndBuilder<(T0, T1, T2, T3, T4)>
                where
                    T4: ::planus::WriteAsDefault<u64, u64>,
                {
                    let (v0, v1, v2, v3) = self.0;
                    GameEndBuilder((v0, v1, v2, v3, value))
                }

                /// Sets the [`score_res_hi` field](GameEnd#structfield.score_res_hi) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn score_res_hi_as_default(
                    self,
                ) -> GameEndBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)> {
                    self.score_res_hi(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4> GameEndBuilder<(T0, T1, T2, T3, T4)> {
                /// Setter for the [`score_res_lo` field](GameEnd#structfield.score_res_lo).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn score_res_lo<T5>(self, value: T5) -> GameEndBuilder<(T0, T1, T2, T3, T4, T5)>
                where
                    T5: ::planus::WriteAsDefault<u64, u64>,
                {
                    let (v0, v1, v2, v3, v4) = self.0;
                    GameEndBuilder((v0, v1, v2, v3, v4, value))
                }

                /// Sets the [`score_res_lo` field](GameEnd#structfield.score_res_lo) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn score_res_lo_as_default(
                    self,
                ) -> GameEndBuilder<(T0, T1, T2, T3, T4, ::planus::DefaultValue)> {
                    self.score_res_lo(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5> GameEndBuilder<(T0, T1, T2, T3, T4, T5)> {
                /// Setter for the [`rounds_completed` field](GameEnd#structfield.rounds_completed).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn rounds_completed<T6>(
                    self,
                    value: T6,
                ) -> GameEndBuilder<(T0, T1, T2, T3, T4, T5, T6)>
                where
                    T6: ::planus::WriteAsDefault<u32, u32>,
                {
                    let (v0, v1, v2, v3, v4, v5) = self.0;
                    GameEndBuilder((v0, v1, v2, v3, v4, v5, value))
                }

                /// Sets the [`rounds_completed` field](GameEnd#structfield.rounds_completed) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn rounds_completed_as_default(
                    self,
                ) -> GameEndBuilder<(T0, T1, T2, T3, T4, T5, ::planus::DefaultValue)>
                {
                    self.rounds_completed(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6> GameEndBuilder<(T0, T1, T2, T3, T4, T5, T6)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [GameEnd].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<GameEnd>
                where
                    Self: ::planus::WriteAsOffset<GameEnd>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                T2: ::planus::WriteAsDefault<u64, u64>,
                T3: ::planus::WriteAsDefault<u64, u64>,
                T4: ::planus::WriteAsDefault<u64, u64>,
                T5: ::planus::WriteAsDefault<u64, u64>,
                T6: ::planus::WriteAsDefault<u32, u32>,
            > ::planus::WriteAs<::planus::Offset<GameEnd>>
                for GameEndBuilder<(T0, T1, T2, T3, T4, T5, T6)>
            {
                type Prepared = ::planus::Offset<GameEnd>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<GameEnd> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                T2: ::planus::WriteAsDefault<u64, u64>,
                T3: ::planus::WriteAsDefault<u64, u64>,
                T4: ::planus::WriteAsDefault<u64, u64>,
                T5: ::planus::WriteAsDefault<u64, u64>,
                T6: ::planus::WriteAsDefault<u32, u32>,
            > ::planus::WriteAsOptional<::planus::Offset<GameEnd>>
                for GameEndBuilder<(T0, T1, T2, T3, T4, T5, T6)>
            {
                type Prepared = ::planus::Offset<GameEnd>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<GameEnd>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                T2: ::planus::WriteAsDefault<u64, u64>,
                T3: ::planus::WriteAsDefault<u64, u64>,
                T4: ::planus::WriteAsDefault<u64, u64>,
                T5: ::planus::WriteAsDefault<u64, u64>,
                T6: ::planus::WriteAsDefault<u32, u32>,
            > ::planus::WriteAsOffset<GameEnd> for GameEndBuilder<(T0, T1, T2, T3, T4, T5, T6)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<GameEnd> {
                    let (v0, v1, v2, v3, v4, v5, v6) = &self.0;
                    GameEnd::create(builder, v0, v1, v2, v3, v4, v5, v6)
                }
            }

            /// Reference to a deserialized [GameEnd].
            #[derive(Copy, Clone)]
            pub struct GameEndRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> GameEndRef<'a> {
                /// Getter for the [`outcome` field](GameEnd#structfield.outcome).
                #[inline]
                pub fn outcome(&self) -> ::planus::Result<u8> {
                    ::core::result::Result::Ok(self.0.access(0, "GameEnd", "outcome")?.unwrap_or(0))
                }

                /// Getter for the [`detail` field](GameEnd#structfield.detail).
                #[inline]
                pub fn detail(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(1, "GameEnd", "detail")
                }

                /// Getter for the [`score_enl_hi` field](GameEnd#structfield.score_enl_hi).
                #[inline]
                pub fn score_enl_hi(&self) -> ::planus::Result<u64> {
                    ::core::result::Result::Ok(
                        self.0.access(2, "GameEnd", "score_enl_hi")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`score_enl_lo` field](GameEnd#structfield.score_enl_lo).
                #[inline]
                pub fn score_enl_lo(&self) -> ::planus::Result<u64> {
                    ::core::result::Result::Ok(
                        self.0.access(3, "GameEnd", "score_enl_lo")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`score_res_hi` field](GameEnd#structfield.score_res_hi).
                #[inline]
                pub fn score_res_hi(&self) -> ::planus::Result<u64> {
                    ::core::result::Result::Ok(
                        self.0.access(4, "GameEnd", "score_res_hi")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`score_res_lo` field](GameEnd#structfield.score_res_lo).
                #[inline]
                pub fn score_res_lo(&self) -> ::planus::Result<u64> {
                    ::core::result::Result::Ok(
                        self.0.access(5, "GameEnd", "score_res_lo")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`rounds_completed` field](GameEnd#structfield.rounds_completed).
                #[inline]
                pub fn rounds_completed(&self) -> ::planus::Result<u32> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(6, "GameEnd", "rounds_completed")?
                            .unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for GameEndRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("GameEndRef");
                    f.field("outcome", &self.outcome());
                    if let ::core::option::Option::Some(field_detail) = self.detail().transpose() {
                        f.field("detail", &field_detail);
                    }
                    f.field("score_enl_hi", &self.score_enl_hi());
                    f.field("score_enl_lo", &self.score_enl_lo());
                    f.field("score_res_hi", &self.score_res_hi());
                    f.field("score_res_lo", &self.score_res_lo());
                    f.field("rounds_completed", &self.rounds_completed());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<GameEndRef<'a>> for GameEnd {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: GameEndRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        outcome: ::core::convert::TryInto::try_into(value.outcome()?)?,
                        detail: value.detail()?.map(::core::convert::Into::into),
                        score_enl_hi: ::core::convert::TryInto::try_into(value.score_enl_hi()?)?,
                        score_enl_lo: ::core::convert::TryInto::try_into(value.score_enl_lo()?)?,
                        score_res_hi: ::core::convert::TryInto::try_into(value.score_res_hi()?)?,
                        score_res_lo: ::core::convert::TryInto::try_into(value.score_res_lo()?)?,
                        rounds_completed: ::core::convert::TryInto::try_into(
                            value.rounds_completed()?,
                        )?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for GameEndRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for GameEndRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[GameEndRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<GameEnd>> for GameEnd {
                type Value = ::planus::Offset<GameEnd>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<GameEnd>],
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

            impl<'a> ::planus::ReadAsRoot<'a> for GameEndRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[GameEndRef]", "read_as_root", 0)
                    })
                }
            }
        }
    }
}
