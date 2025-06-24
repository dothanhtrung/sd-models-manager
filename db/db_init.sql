create table app_info
(
    label TEXT    not null,
    value integer not null
);

create table base
(
    id         integer              not null
        constraint base_pk_2
            primary key autoincrement,
    label      TEXT                 not null
        constraint base_pk
            unique,
    is_checked integer default true not null
);

create table item
(
    id         integer              not null
        constraint item_pk
            primary key autoincrement,
    path       TEXT                 not null,
    base_id    integer              not null
        constraint item_base_id_fk
            references base
            on delete cascade,
    hash       TEXT,
    is_checked integer default true not null,
    parent     integer
        constraint item_item_id_fk
            references item
            on delete cascade,
    name       TEXT
);

create table tag
(
    id          integer not null
        constraint tag_pk
            primary key autoincrement,
    name        TEXT    not null
        constraint tag_pk_2
            unique,
    description integer
);

create unique index tag_name_uindex
    on tag (name);

create table tag_item
(
    tag  integer
        constraint tag_item_tag_id_fk
            references tag
            on delete cascade,
    item integer
        constraint tag_model_model_id_fk
            references item
            on delete cascade
);

create unique index tag_item_item_tag_uindex
    on tag_item (item, tag);

