def hash
  ([@addr, @mask_addr, @zone_id].hash << 1) | (ipv4? ? 0 : 1)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `[...].hash` instead of combining hash values manually.
end
