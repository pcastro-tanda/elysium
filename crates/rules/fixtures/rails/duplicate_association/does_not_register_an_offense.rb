        class Post < ActiveModel::Serializer
          has_many :comments, key: :remarks, if: :formal_mode?
          has_many :comments, key: :rejoinders, if: :debate_mode?
        end
