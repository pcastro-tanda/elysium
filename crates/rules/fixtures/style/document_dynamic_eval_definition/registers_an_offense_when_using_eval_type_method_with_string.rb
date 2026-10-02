class_eval <<-EOT, __FILE__, __LINE__ + 1
^^^^^^^^^^ Add a comment block showing its appearance if interpolated.
  def #{unsafe_method}(*params, &block)
    to_str.#{unsafe_method}(*params, &block)
  end
EOT
