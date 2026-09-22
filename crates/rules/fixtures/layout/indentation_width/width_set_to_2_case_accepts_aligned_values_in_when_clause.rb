case superclass
when /\A(#{NAMESPACEMATCH})(?:\s|\Z)/,
     /\A(Struct|OStruct)\.new/,
     /\ADelegateClass\((.+?)\)\s*\Z/,
     /\A(#{NAMESPACEMATCH})\(/
  $1
when "self"
  namespace.path
end
