assert_difference(MyModel.count, +2,
                  3,  +3, # Extra spacing only here.
                    ^ Unnecessary spacing detected.
                  4,+4)
