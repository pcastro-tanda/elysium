def client_info_hash
  @client_info_hash = T.let(@client_info_hash, T.nilable(T::Hash[Symbol, T.untyped]))
  @client_info_hash ||= client_info.to_hash
end
