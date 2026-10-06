class User < T::Struct
  const :first_name, String
        ^^^^^^^^^^^ Use camelCase for T::Struct property names.
  prop :last_name, String
       ^^^^^^^^^^ Use camelCase for T::Struct property names.
  const :firstName, String
  prop :lastName, String
end
