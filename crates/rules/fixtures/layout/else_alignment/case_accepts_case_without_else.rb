case superclass
when /\A(#{NAMESPACEMATCH})(?:\s|\Z)/
  $1
when "self"
  namespace.path
end
