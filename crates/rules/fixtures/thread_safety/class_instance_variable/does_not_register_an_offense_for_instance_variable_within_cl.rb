def separate_with(separator)
  Example.class_eval do
    def separator
      @separator
    end
  end
end
