def separate_with(separator)
  Utilities.module_exec do
    @separator = separator
  end
end
