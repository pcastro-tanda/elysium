def m(x, y = 1, *args, required:, optional: true, **kwargs, &block)
  return to_enum(__method__, x, y, *args, required: required, optional: optional, **kwargs) unless block_given?
end
