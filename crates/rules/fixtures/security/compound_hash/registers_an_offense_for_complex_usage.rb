def hash
  @hash ||= begin
    hash_ = [@factory, geometry_type].hash
    (0...num_geometries).inject(hash_) do |h_, i_|
      (1664525 * h_ + geometry_n(i_).hash).hash
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `[...].hash` instead of combining hash values manually.
    end
  end
end
