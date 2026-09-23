type_name ||= value.class.name if value
type_name   = type_name.to_s   if type_name
                            ^^ Unnecessary spacing detected.
         ^^ Unnecessary spacing detected.

type_name  = value.class.name if     value
                                ^^^^ Unnecessary spacing detected.
         ^ Unnecessary spacing detected.
type_name += type_name.to_s   unless type_name
                           ^^ Unnecessary spacing detected.
a  += 1
 ^ Unnecessary spacing detected.
aa -= 2
