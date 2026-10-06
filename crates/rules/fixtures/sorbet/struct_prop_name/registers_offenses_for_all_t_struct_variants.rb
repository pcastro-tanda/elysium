class User < T::ImmutableStruct
  const :firstName, String
        ^^^^^^^^^^ Use snake_case for T::Struct property names.
end

class Account < ::T::InexactStruct
  prop :accountId, Integer
       ^^^^^^^^^^ Use snake_case for T::Struct property names.
end
