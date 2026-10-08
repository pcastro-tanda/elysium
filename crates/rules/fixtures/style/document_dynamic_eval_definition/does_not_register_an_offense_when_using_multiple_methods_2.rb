class_eval(
  # def capitalize(*params, &block)
  #   to_str.capitalize(*params, &block)
  # end
  #
  # def capitalize!(*params)
  #   @dirty = true
  #   super
  # end

  <<-EOT, __FILE__, __LINE__ + 1
    def #{unsafe_method}(*params, &block)
      to_str.#{unsafe_method}(*params, &block)
    end

    def #{unsafe_method}!(*params)
      @dirty = true
      super
    end
  EOT
)
