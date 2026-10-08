class User < T::Struct
  const :firstName, String
        ^^^^^^^^^^ Use snake_case for T::Struct property names.
  prop :lastName, String
       ^^^^^^^^^ Use snake_case for T::Struct property names.
  self.const :middleName, String
             ^^^^^^^^^^^ Use snake_case for T::Struct property names.
  self.prop :displayName, String
            ^^^^^^^^^^^^ Use snake_case for T::Struct property names.
  const :first_name, String
  prop :last_name, String
end
