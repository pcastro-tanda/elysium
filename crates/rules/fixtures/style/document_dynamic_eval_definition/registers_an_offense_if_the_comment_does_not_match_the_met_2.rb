class_eval(
^^^^^^^^^^ Add a comment block showing its appearance if interpolated.
  # def capitalize(*params, &block)
  #   str.capitalize(*params, &block)
  # end

  <<-EOT, __FILE__, __LINE__ + 1
    def #{unsafe_method}(*params, &block)
      to_str.#{unsafe_method}(*params, &block)
    end
  EOT
)
