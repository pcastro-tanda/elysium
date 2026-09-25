def const_name(node)
  const_names = []
  const_node = node

  loop do
    namespace_node, name = *const_node
    const_names << name
    break unless namespace_node
    break if namespace_node.type == :cbase
    const_node = namespace_node
  end

  const_names.reverse.join('::')
end
