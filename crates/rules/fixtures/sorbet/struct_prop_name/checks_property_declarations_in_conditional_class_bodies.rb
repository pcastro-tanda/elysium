class User < T::Struct
  if feature_enabled?
    const :displayName, String
          ^^^^^^^^^^^^ Use snake_case for T::Struct property names.
  end
end
