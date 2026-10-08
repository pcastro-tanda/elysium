def client_info_hash
  @client_info_hash = T.let(@client_info_hash, T.nilable(T::Hash[Symbol, T.untyped]))
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/ObsoleteStrictMemoization: This two-stage workaround for memoization in `#typed: strict` files is no longer necessary. See https://sorbet.org/docs/type-assertions#put-type-assertions-behind-memoization.
  @client_info_hash ||= client_info.to_hash
end
