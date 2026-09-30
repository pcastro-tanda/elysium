# Aligned with start of line where do is:
params = default_options.merge(options)
          .delete_if { |k, v| v.nil? }
          .each_with_object({}) do |(k, v), new_hash|
            new_hash[k.to_s] = v.to_s
          end
# Aligned with start of the whole expression:
params = default_options.merge(options)
          .delete_if { |k, v| v.nil? }
          .each_with_object({}) do |(k, v), new_hash|
            new_hash[k.to_s] = v.to_s
end
