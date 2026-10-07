class User < T::Struct
  class Profile < T::Struct
    const :displayName, String
          ^^^^^^^^^^^^ Use snake_case for T::Struct property names.
  end
end
