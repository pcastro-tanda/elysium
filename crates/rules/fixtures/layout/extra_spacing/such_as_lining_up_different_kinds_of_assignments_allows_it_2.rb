type_name ||= value.class.name if value
type_name   = type_name.to_s   if type_name

type_name  = value.class.name if     value
type_name += type_name.to_s   unless type_name
a  += 1
aa -= 2
