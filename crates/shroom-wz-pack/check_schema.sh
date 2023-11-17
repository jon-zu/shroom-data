#!/bin/zsh

#FILE_LIST=$(find * -type f -regex 'out2/items/[a-zA-Z]+/[0-9]+\.img/img\.json'  | paste -sd " ")

FILE_LIST=$(find * -type f -regex 'out2/items/Pet/[0-9]+.img/img\.json'  | paste -sd " ")

eval ~/.local/bin/check-jsonschema --schemafile schemas/pet_item.schema.json $FILE_LIST

