def hash
  [foo&.hash, bar&.hash].hash
              ^^^^^^^^^ Calling .hash on elements of a hashed array is redundant.
   ^^^^^^^^^ Calling .hash on elements of a hashed array is redundant.
end
