pub use root::*;

const _: () = ::planus::check_version_compatibility("planus-1.3.0");

/// The root namespace
///
/// Generated from these locations:
/// * File `crates/arena-protocol/schemas/private_snapshot.fbs`
#[no_implicit_prelude]
#[allow(clippy::needless_lifetimes)]
mod root {
    /// The namespace `ingress`
    ///
    /// Generated from these locations:
    /// * File `crates/arena-protocol/schemas/private_snapshot.fbs`
    pub mod ingress {
        /// The namespace `ingress.snapshot`
        ///
        /// Generated from these locations:
        /// * File `crates/arena-protocol/schemas/private_snapshot.fbs`
        pub mod snapshot {
            /// The namespace `ingress.snapshot.private`
            ///
            /// Generated from these locations:
            /// * File `crates/arena-protocol/schemas/private_snapshot.fbs`
            pub mod private {
                /// The table `KeyCount` in the namespace `ingress.snapshot.private`
                ///
                /// Generated from these locations:
                /// * Table `KeyCount` in the file `crates/arena-protocol/schemas/private_snapshot.fbs:15`
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
                                if let ::core::option::Option::Some(prepared_portal) =
                                    prepared_portal
                                {
                                    object_writer.write::<_, _, 4>(&prepared_portal);
                                }
                                if let ::core::option::Option::Some(prepared_count) = prepared_count
                                {
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
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<KeyCount> {
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
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<KeyCount> for KeyCount {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<KeyCount> {
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
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<KeyCount>
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
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<KeyCount> {
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
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl<T0: ::planus::WriteAsDefault<u32, u32>, T1: ::planus::WriteAsDefault<u32, u32>>
                    ::planus::WriteAsOffset<KeyCount> for KeyCountBuilder<(T0, T1)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<KeyCount> {
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
                        ::core::result::Result::Ok(
                            self.0.access(0, "KeyCount", "portal")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`count` field](KeyCount#structfield.count).
                    #[inline]
                    pub fn count(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "KeyCount", "count")?.unwrap_or(0),
                        )
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
                    ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind>
                    {
                        ::core::result::Result::Ok(Self(
                            ::planus::table_reader::Table::from_buffer(buffer, offset)?,
                        ))
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

                /// The table `AgentEntry` in the namespace `ingress.snapshot.private`
                ///
                /// Generated from these locations:
                /// * Table `AgentEntry` in the file `crates/arena-protocol/schemas/private_snapshot.fbs:20`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct AgentEntry {
                    /// The field `id` in the table `AgentEntry`
                    pub id: u32,
                    /// The field `x` in the table `AgentEntry`
                    pub x: i32,
                    /// The field `y` in the table `AgentEntry`
                    pub y: i32,
                    /// The field `supply` in the table `AgentEntry`
                    pub supply: u32,
                    /// The field `keys` in the table `AgentEntry`
                    pub keys: ::core::option::Option<::planus::alloc::vec::Vec<self::KeyCount>>,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for AgentEntry {
                    fn default() -> Self {
                        Self {
                            id: 0,
                            x: 0,
                            y: 0,
                            supply: 0,
                            keys: ::core::default::Default::default(),
                        }
                    }
                }

                impl AgentEntry {
                    /// Creates a [AgentEntryBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> AgentEntryBuilder<()> {
                        AgentEntryBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_id: impl ::planus::WriteAsDefault<u32, u32>,
                        field_x: impl ::planus::WriteAsDefault<i32, i32>,
                        field_y: impl ::planus::WriteAsDefault<i32, i32>,
                        field_supply: impl ::planus::WriteAsDefault<u32, u32>,
                        field_keys: impl ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::KeyCount>]>,
                        >,
                    ) -> ::planus::Offset<Self> {
                        let prepared_id = field_id.prepare(builder, &0);
                        let prepared_x = field_x.prepare(builder, &0);
                        let prepared_y = field_y.prepare(builder, &0);
                        let prepared_supply = field_supply.prepare(builder, &0);
                        let prepared_keys = field_keys.prepare(builder);

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
                        if prepared_supply.is_some() {
                            table_writer.write_entry::<u32>(3);
                        }
                        if prepared_keys.is_some() {
                            table_writer.write_entry::<::planus::Offset<[::planus::Offset<self::KeyCount>]>>(4);
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
                                if let ::core::option::Option::Some(prepared_supply) =
                                    prepared_supply
                                {
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

                impl ::planus::WriteAs<::planus::Offset<AgentEntry>> for AgentEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<AgentEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<AgentEntry>> for AgentEntry {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<AgentEntry>> {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<AgentEntry> for AgentEntry {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<AgentEntry> {
                        AgentEntry::create(
                            builder,
                            self.id,
                            self.x,
                            self.y,
                            self.supply,
                            &self.keys,
                        )
                    }
                }

                /// Builder for serializing an instance of the [AgentEntry] type.
                ///
                /// Can be created using the [AgentEntry::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct AgentEntryBuilder<State>(State);

                impl AgentEntryBuilder<()> {
                    /// Setter for the [`id` field](AgentEntry#structfield.id).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn id<T0>(self, value: T0) -> AgentEntryBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u32, u32>,
                    {
                        AgentEntryBuilder((value,))
                    }

                    /// Sets the [`id` field](AgentEntry#structfield.id) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn id_as_default(self) -> AgentEntryBuilder<(::planus::DefaultValue,)> {
                        self.id(::planus::DefaultValue)
                    }
                }

                impl<T0> AgentEntryBuilder<(T0,)> {
                    /// Setter for the [`x` field](AgentEntry#structfield.x).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn x<T1>(self, value: T1) -> AgentEntryBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<i32, i32>,
                    {
                        let (v0,) = self.0;
                        AgentEntryBuilder((v0, value))
                    }

                    /// Sets the [`x` field](AgentEntry#structfield.x) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn x_as_default(self) -> AgentEntryBuilder<(T0, ::planus::DefaultValue)> {
                        self.x(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> AgentEntryBuilder<(T0, T1)> {
                    /// Setter for the [`y` field](AgentEntry#structfield.y).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn y<T2>(self, value: T2) -> AgentEntryBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<i32, i32>,
                    {
                        let (v0, v1) = self.0;
                        AgentEntryBuilder((v0, v1, value))
                    }

                    /// Sets the [`y` field](AgentEntry#structfield.y) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn y_as_default(
                        self,
                    ) -> AgentEntryBuilder<(T0, T1, ::planus::DefaultValue)> {
                        self.y(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> AgentEntryBuilder<(T0, T1, T2)> {
                    /// Setter for the [`supply` field](AgentEntry#structfield.supply).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn supply<T3>(self, value: T3) -> AgentEntryBuilder<(T0, T1, T2, T3)>
                    where
                        T3: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0, v1, v2) = self.0;
                        AgentEntryBuilder((v0, v1, v2, value))
                    }

                    /// Sets the [`supply` field](AgentEntry#structfield.supply) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn supply_as_default(
                        self,
                    ) -> AgentEntryBuilder<(T0, T1, T2, ::planus::DefaultValue)>
                    {
                        self.supply(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2, T3> AgentEntryBuilder<(T0, T1, T2, T3)> {
                    /// Setter for the [`keys` field](AgentEntry#structfield.keys).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn keys<T4>(self, value: T4) -> AgentEntryBuilder<(T0, T1, T2, T3, T4)>
                    where
                        T4: ::planus::WriteAsOptional<
                                ::planus::Offset<[::planus::Offset<self::KeyCount>]>,
                            >,
                    {
                        let (v0, v1, v2, v3) = self.0;
                        AgentEntryBuilder((v0, v1, v2, v3, value))
                    }

                    /// Sets the [`keys` field](AgentEntry#structfield.keys) to null.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn keys_as_null(self) -> AgentEntryBuilder<(T0, T1, T2, T3, ())> {
                        self.keys(())
                    }
                }

                impl<T0, T1, T2, T3, T4> AgentEntryBuilder<(T0, T1, T2, T3, T4)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [AgentEntry].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<AgentEntry>
                    where
                        Self: ::planus::WriteAsOffset<AgentEntry>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<i32, i32>,
                    T2: ::planus::WriteAsDefault<i32, i32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::KeyCount>]>>,
                > ::planus::WriteAs<::planus::Offset<AgentEntry>>
                    for AgentEntryBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<AgentEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<AgentEntry> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<i32, i32>,
                    T2: ::planus::WriteAsDefault<i32, i32>,
                    T3: ::planus::WriteAsDefault<u32, u32>,
                    T4: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::KeyCount>]>>,
                > ::planus::WriteAsOptional<::planus::Offset<AgentEntry>>
                    for AgentEntryBuilder<(T0, T1, T2, T3, T4)>
                {
                    type Prepared = ::planus::Offset<AgentEntry>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<AgentEntry>> {
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
                    T4: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::KeyCount>]>>,
                > ::planus::WriteAsOffset<AgentEntry> for AgentEntryBuilder<(T0, T1, T2, T3, T4)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<AgentEntry> {
                        let (v0, v1, v2, v3, v4) = &self.0;
                        AgentEntry::create(builder, v0, v1, v2, v3, v4)
                    }
                }

                /// Reference to a deserialized [AgentEntry].
                #[derive(Copy, Clone)]
                pub struct AgentEntryRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

                impl<'a> AgentEntryRef<'a> {
                    /// Getter for the [`id` field](AgentEntry#structfield.id).
                    #[inline]
                    pub fn id(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(0, "AgentEntry", "id")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`x` field](AgentEntry#structfield.x).
                    #[inline]
                    pub fn x(&self) -> ::planus::Result<i32> {
                        ::core::result::Result::Ok(
                            self.0.access(1, "AgentEntry", "x")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`y` field](AgentEntry#structfield.y).
                    #[inline]
                    pub fn y(&self) -> ::planus::Result<i32> {
                        ::core::result::Result::Ok(
                            self.0.access(2, "AgentEntry", "y")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`supply` field](AgentEntry#structfield.supply).
                    #[inline]
                    pub fn supply(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0.access(3, "AgentEntry", "supply")?.unwrap_or(0),
                        )
                    }

                    /// Getter for the [`keys` field](AgentEntry#structfield.keys).
                    #[inline]
                    pub fn keys(
                        &self,
                    ) -> ::planus::Result<
                        ::core::option::Option<
                            ::planus::Vector<'a, ::planus::Result<self::KeyCountRef<'a>>>,
                        >,
                    > {
                        self.0.access(4, "AgentEntry", "keys")
                    }
                }

                impl<'a> ::core::fmt::Debug for AgentEntryRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("AgentEntryRef");
                        f.field("id", &self.id());
                        f.field("x", &self.x());
                        f.field("y", &self.y());
                        f.field("supply", &self.supply());
                        if let ::core::option::Option::Some(field_keys) = self.keys().transpose() {
                            f.field("keys", &field_keys);
                        }
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<AgentEntryRef<'a>> for AgentEntry {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: AgentEntryRef<'a>) -> ::planus::Result<Self> {
                        ::core::result::Result::Ok(Self {
                            id: ::core::convert::TryInto::try_into(value.id()?)?,
                            x: ::core::convert::TryInto::try_into(value.x()?)?,
                            y: ::core::convert::TryInto::try_into(value.y()?)?,
                            supply: ::core::convert::TryInto::try_into(value.supply()?)?,
                            keys: if let ::core::option::Option::Some(keys) = value.keys()? {
                                ::core::option::Option::Some(keys.to_vec_result()?)
                            } else {
                                ::core::option::Option::None
                            },
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for AgentEntryRef<'a> {
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

                impl<'a> ::planus::VectorReadInner<'a> for AgentEntryRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[AgentEntryRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<AgentEntry>> for AgentEntry {
                    type Value = ::planus::Offset<AgentEntry>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<AgentEntry>],
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

                impl<'a> ::planus::ReadAsRoot<'a> for AgentEntryRef<'a> {
                    fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(
                            ::planus::SliceWithStartOffset {
                                buffer: slice,
                                offset_from_start: 0,
                            },
                            0,
                        )
                        .map_err(|error_kind| {
                            error_kind.with_error_location("[AgentEntryRef]", "read_as_root", 0)
                        })
                    }
                }

                /// The table `PrivateSnapshot` in the namespace `ingress.snapshot.private`
                ///
                /// Generated from these locations:
                /// * Table `PrivateSnapshot` in the file `crates/arena-protocol/schemas/private_snapshot.fbs:28`
                #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
                pub struct PrivateSnapshot {
                    /// The field `schema_version` in the table `PrivateSnapshot`
                    pub schema_version: u32,
                    /// The field `current_round` in the table `PrivateSnapshot`
                    pub current_round: u32,
                    /// The field `snapshot_version` in the table `PrivateSnapshot`
                    pub snapshot_version: u64,
                    /// The field `agents` in the table `PrivateSnapshot`
                    pub agents: ::core::option::Option<::planus::alloc::vec::Vec<self::AgentEntry>>,
                }

                #[allow(clippy::derivable_impls)]
                impl ::core::default::Default for PrivateSnapshot {
                    fn default() -> Self {
                        Self {
                            schema_version: 1,
                            current_round: 0,
                            snapshot_version: 0,
                            agents: ::core::default::Default::default(),
                        }
                    }
                }

                impl PrivateSnapshot {
                    /// Creates a [PrivateSnapshotBuilder] for serializing an instance of this table.
                    #[inline]
                    pub fn builder() -> PrivateSnapshotBuilder<()> {
                        PrivateSnapshotBuilder(())
                    }

                    #[allow(clippy::too_many_arguments)]
                    pub fn create(
                        builder: &mut ::planus::Builder,
                        field_schema_version: impl ::planus::WriteAsDefault<u32, u32>,
                        field_current_round: impl ::planus::WriteAsDefault<u32, u32>,
                        field_snapshot_version: impl ::planus::WriteAsDefault<u64, u64>,
                        field_agents: impl ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::AgentEntry>]>,
                        >,
                    ) -> ::planus::Offset<Self> {
                        let prepared_schema_version = field_schema_version.prepare(builder, &1);
                        let prepared_current_round = field_current_round.prepare(builder, &0);
                        let prepared_snapshot_version = field_snapshot_version.prepare(builder, &0);
                        let prepared_agents = field_agents.prepare(builder);

                        let mut table_writer: ::planus::table_writer::TableWriter<12> =
                            ::core::default::Default::default();
                        if prepared_snapshot_version.is_some() {
                            table_writer.write_entry::<u64>(2);
                        }
                        if prepared_schema_version.is_some() {
                            table_writer.write_entry::<u32>(0);
                        }
                        if prepared_current_round.is_some() {
                            table_writer.write_entry::<u32>(1);
                        }
                        if prepared_agents.is_some() {
                            table_writer.write_entry::<::planus::Offset<[::planus::Offset<self::AgentEntry>]>>(3);
                        }

                        unsafe {
                            table_writer.finish(builder, |object_writer| {
                                if let ::core::option::Option::Some(prepared_snapshot_version) =
                                    prepared_snapshot_version
                                {
                                    object_writer.write::<_, _, 8>(&prepared_snapshot_version);
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
                                if let ::core::option::Option::Some(prepared_agents) =
                                    prepared_agents
                                {
                                    object_writer.write::<_, _, 4>(&prepared_agents);
                                }
                            });
                        }
                        builder.current_offset()
                    }
                }

                impl ::planus::WriteAs<::planus::Offset<PrivateSnapshot>> for PrivateSnapshot {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PrivateSnapshot> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl ::planus::WriteAsOptional<::planus::Offset<PrivateSnapshot>> for PrivateSnapshot {
                    type Prepared = ::planus::Offset<Self>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<PrivateSnapshot>>
                    {
                        ::core::option::Option::Some(::planus::WriteAsOffset::prepare(
                            self, builder,
                        ))
                    }
                }

                impl ::planus::WriteAsOffset<PrivateSnapshot> for PrivateSnapshot {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PrivateSnapshot> {
                        PrivateSnapshot::create(
                            builder,
                            self.schema_version,
                            self.current_round,
                            self.snapshot_version,
                            &self.agents,
                        )
                    }
                }

                /// Builder for serializing an instance of the [PrivateSnapshot] type.
                ///
                /// Can be created using the [PrivateSnapshot::builder] method.
                #[derive(Debug)]
                #[must_use]
                pub struct PrivateSnapshotBuilder<State>(State);

                impl PrivateSnapshotBuilder<()> {
                    /// Setter for the [`schema_version` field](PrivateSnapshot#structfield.schema_version).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn schema_version<T0>(self, value: T0) -> PrivateSnapshotBuilder<(T0,)>
                    where
                        T0: ::planus::WriteAsDefault<u32, u32>,
                    {
                        PrivateSnapshotBuilder((value,))
                    }

                    /// Sets the [`schema_version` field](PrivateSnapshot#structfield.schema_version) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn schema_version_as_default(
                        self,
                    ) -> PrivateSnapshotBuilder<(::planus::DefaultValue,)> {
                        self.schema_version(::planus::DefaultValue)
                    }
                }

                impl<T0> PrivateSnapshotBuilder<(T0,)> {
                    /// Setter for the [`current_round` field](PrivateSnapshot#structfield.current_round).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn current_round<T1>(self, value: T1) -> PrivateSnapshotBuilder<(T0, T1)>
                    where
                        T1: ::planus::WriteAsDefault<u32, u32>,
                    {
                        let (v0,) = self.0;
                        PrivateSnapshotBuilder((v0, value))
                    }

                    /// Sets the [`current_round` field](PrivateSnapshot#structfield.current_round) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn current_round_as_default(
                        self,
                    ) -> PrivateSnapshotBuilder<(T0, ::planus::DefaultValue)> {
                        self.current_round(::planus::DefaultValue)
                    }
                }

                impl<T0, T1> PrivateSnapshotBuilder<(T0, T1)> {
                    /// Setter for the [`snapshot_version` field](PrivateSnapshot#structfield.snapshot_version).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn snapshot_version<T2>(
                        self,
                        value: T2,
                    ) -> PrivateSnapshotBuilder<(T0, T1, T2)>
                    where
                        T2: ::planus::WriteAsDefault<u64, u64>,
                    {
                        let (v0, v1) = self.0;
                        PrivateSnapshotBuilder((v0, v1, value))
                    }

                    /// Sets the [`snapshot_version` field](PrivateSnapshot#structfield.snapshot_version) to the default value.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn snapshot_version_as_default(
                        self,
                    ) -> PrivateSnapshotBuilder<(T0, T1, ::planus::DefaultValue)>
                    {
                        self.snapshot_version(::planus::DefaultValue)
                    }
                }

                impl<T0, T1, T2> PrivateSnapshotBuilder<(T0, T1, T2)> {
                    /// Setter for the [`agents` field](PrivateSnapshot#structfield.agents).
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agents<T3>(self, value: T3) -> PrivateSnapshotBuilder<(T0, T1, T2, T3)>
                    where
                        T3: ::planus::WriteAsOptional<
                                ::planus::Offset<[::planus::Offset<self::AgentEntry>]>,
                            >,
                    {
                        let (v0, v1, v2) = self.0;
                        PrivateSnapshotBuilder((v0, v1, v2, value))
                    }

                    /// Sets the [`agents` field](PrivateSnapshot#structfield.agents) to null.
                    #[inline]
                    #[allow(clippy::type_complexity)]
                    pub fn agents_as_null(self) -> PrivateSnapshotBuilder<(T0, T1, T2, ())> {
                        self.agents(())
                    }
                }

                impl<T0, T1, T2, T3> PrivateSnapshotBuilder<(T0, T1, T2, T3)> {
                    /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [PrivateSnapshot].
                    #[inline]
                    pub fn finish(
                        self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PrivateSnapshot>
                    where
                        Self: ::planus::WriteAsOffset<PrivateSnapshot>,
                    {
                        ::planus::WriteAsOffset::prepare(&self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u64, u64>,
                    T3: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::AgentEntry>]>,
                        >,
                > ::planus::WriteAs<::planus::Offset<PrivateSnapshot>>
                    for PrivateSnapshotBuilder<(T0, T1, T2, T3)>
                {
                    type Prepared = ::planus::Offset<PrivateSnapshot>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PrivateSnapshot> {
                        ::planus::WriteAsOffset::prepare(self, builder)
                    }
                }

                impl<
                    T0: ::planus::WriteAsDefault<u32, u32>,
                    T1: ::planus::WriteAsDefault<u32, u32>,
                    T2: ::planus::WriteAsDefault<u64, u64>,
                    T3: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::AgentEntry>]>,
                        >,
                > ::planus::WriteAsOptional<::planus::Offset<PrivateSnapshot>>
                    for PrivateSnapshotBuilder<(T0, T1, T2, T3)>
                {
                    type Prepared = ::planus::Offset<PrivateSnapshot>;

                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::core::option::Option<::planus::Offset<PrivateSnapshot>>
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
                    T3: ::planus::WriteAsOptional<
                            ::planus::Offset<[::planus::Offset<self::AgentEntry>]>,
                        >,
                > ::planus::WriteAsOffset<PrivateSnapshot>
                    for PrivateSnapshotBuilder<(T0, T1, T2, T3)>
                {
                    #[inline]
                    fn prepare(
                        &self,
                        builder: &mut ::planus::Builder,
                    ) -> ::planus::Offset<PrivateSnapshot> {
                        let (v0, v1, v2, v3) = &self.0;
                        PrivateSnapshot::create(builder, v0, v1, v2, v3)
                    }
                }

                /// Reference to a deserialized [PrivateSnapshot].
                #[derive(Copy, Clone)]
                pub struct PrivateSnapshotRef<'a>(
                    #[allow(dead_code)] ::planus::table_reader::Table<'a>,
                );

                impl<'a> PrivateSnapshotRef<'a> {
                    /// Getter for the [`schema_version` field](PrivateSnapshot#structfield.schema_version).
                    #[inline]
                    pub fn schema_version(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(0, "PrivateSnapshot", "schema_version")?
                                .unwrap_or(1),
                        )
                    }

                    /// Getter for the [`current_round` field](PrivateSnapshot#structfield.current_round).
                    #[inline]
                    pub fn current_round(&self) -> ::planus::Result<u32> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(1, "PrivateSnapshot", "current_round")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`snapshot_version` field](PrivateSnapshot#structfield.snapshot_version).
                    #[inline]
                    pub fn snapshot_version(&self) -> ::planus::Result<u64> {
                        ::core::result::Result::Ok(
                            self.0
                                .access(2, "PrivateSnapshot", "snapshot_version")?
                                .unwrap_or(0),
                        )
                    }

                    /// Getter for the [`agents` field](PrivateSnapshot#structfield.agents).
                    #[inline]
                    pub fn agents(
                        &self,
                    ) -> ::planus::Result<
                        ::core::option::Option<
                            ::planus::Vector<'a, ::planus::Result<self::AgentEntryRef<'a>>>,
                        >,
                    > {
                        self.0.access(3, "PrivateSnapshot", "agents")
                    }
                }

                impl<'a> ::core::fmt::Debug for PrivateSnapshotRef<'a> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let mut f = f.debug_struct("PrivateSnapshotRef");
                        f.field("schema_version", &self.schema_version());
                        f.field("current_round", &self.current_round());
                        f.field("snapshot_version", &self.snapshot_version());
                        if let ::core::option::Option::Some(field_agents) =
                            self.agents().transpose()
                        {
                            f.field("agents", &field_agents);
                        }
                        f.finish()
                    }
                }

                impl<'a> ::core::convert::TryFrom<PrivateSnapshotRef<'a>> for PrivateSnapshot {
                    type Error = ::planus::Error;

                    #[allow(unreachable_code)]
                    fn try_from(value: PrivateSnapshotRef<'a>) -> ::planus::Result<Self> {
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
                            agents: if let ::core::option::Option::Some(agents) = value.agents()? {
                                ::core::option::Option::Some(agents.to_vec_result()?)
                            } else {
                                ::core::option::Option::None
                            },
                        })
                    }
                }

                impl<'a> ::planus::TableRead<'a> for PrivateSnapshotRef<'a> {
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

                impl<'a> ::planus::VectorReadInner<'a> for PrivateSnapshotRef<'a> {
                    type Error = ::planus::Error;
                    const STRIDE: usize = 4;

                    unsafe fn from_buffer(
                        buffer: ::planus::SliceWithStartOffset<'a>,
                        offset: usize,
                    ) -> ::planus::Result<Self> {
                        ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                            error_kind.with_error_location(
                                "[PrivateSnapshotRef]",
                                "get",
                                buffer.offset_from_start,
                            )
                        })
                    }
                }

                /// # Safety
                /// The planus compiler generates implementations that initialize
                /// the bytes in `write_values`.
                unsafe impl ::planus::VectorWrite<::planus::Offset<PrivateSnapshot>> for PrivateSnapshot {
                    type Value = ::planus::Offset<PrivateSnapshot>;
                    const STRIDE: usize = 4;
                    #[inline]
                    fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                        ::planus::WriteAs::prepare(self, builder)
                    }

                    #[inline]
                    unsafe fn write_values(
                        values: &[::planus::Offset<PrivateSnapshot>],
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

                impl<'a> ::planus::ReadAsRoot<'a> for PrivateSnapshotRef<'a> {
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
                                "[PrivateSnapshotRef]",
                                "read_as_root",
                                0,
                            )
                        })
                    }
                }
            }
        }
    }
}
