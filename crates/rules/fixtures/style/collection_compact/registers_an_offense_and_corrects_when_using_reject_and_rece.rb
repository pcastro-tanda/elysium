def foo(params)
  params.reject { |_k, v| v.nil? }
         ^^^^^^^^^^^^^^^^^^^^^^^^^ Use `compact` instead of `reject { |_k, v| v.nil? }`.
end
