"this is the #{result&.to_s}"
                       ^^^^ Redundant use of `Object#to_s` in interpolation.
/regexp #{result&.to_s}/
                  ^^^^ Redundant use of `Object#to_s` in interpolation.
:"symbol #{result&.to_s}"
                   ^^^^ Redundant use of `Object#to_s` in interpolation.
`backticks #{result&.to_s}`
                     ^^^^ Redundant use of `Object#to_s` in interpolation.
