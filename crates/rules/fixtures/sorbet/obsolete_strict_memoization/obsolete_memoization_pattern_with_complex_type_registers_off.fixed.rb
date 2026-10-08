def client_info_hash
  @client_info_hash ||= T.let(client_info.to_hash, T.nilable(T::Hash[Symbol, T.untyped]))
end
