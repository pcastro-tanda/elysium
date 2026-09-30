arg_array.size == a.size && (
  arg_array == a ||
  arg_array.map(&:children) == a.map(&:children)
)
